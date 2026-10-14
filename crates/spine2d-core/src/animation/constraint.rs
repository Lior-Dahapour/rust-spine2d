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

use super::curve::CurveTimeline;
use super::{search, ApplyOptions, MixFrom, Property, PropertyId};
use crate::constraint::ConstraintBody;
use crate::constraint_data::{ConstraintData, PhysicsConstraintData, PhysicsConstraintPose};
use crate::skeleton::Skeleton;
use smallvec::{smallvec, SmallVec};

/// Property id for a constraint index; `None` (all physics constraints) is -1 in the reference,
/// which sets every low bit.
fn constraint_property_id(property: Property, constraint: Option<usize>) -> PropertyId {
    match constraint {
        Some(i) => property.id(i),
        None => ((property as u64) << 32) | u64::MAX,
    }
}

/// Changes the mix, softness, bend direction, compress and stretch of an IK constraint.
#[derive(Clone, Debug)]
pub struct IkConstraintTimeline {
    pub curve: CurveTimeline,
    pub constraint: usize,
}

impl IkConstraintTimeline {
    const ENTRIES: usize = 6;

    pub fn new(frame_count: usize, bezier_count: usize, constraint: usize) -> Self {
        IkConstraintTimeline {
            curve: CurveTimeline::new(frame_count, Self::ENTRIES, bezier_count),
            constraint,
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub fn set_frame(
        &mut self,
        frame: usize,
        time: f32,
        mix: f32,
        softness: f32,
        bend_direction: i32,
        compress: bool,
        stretch: bool,
    ) {
        let i = frame * Self::ENTRIES;
        let f = &mut self.curve.frames;
        f[i] = time;
        f[i + 1] = mix;
        f[i + 2] = softness;
        f[i + 3] = bend_direction as f32;
        f[i + 4] = if compress { 1.0 } else { 0.0 };
        f[i + 5] = if stretch { 1.0 } else { 0.0 };
    }

    pub(crate) fn property_ids(&self) -> SmallVec<[PropertyId; 3]> {
        smallvec![Property::IkConstraint.id(self.constraint)]
    }

    pub(crate) fn apply(&self, sk: &mut Skeleton, time: f32, o: &ApplyOptions) {
        let ConstraintData::Ik(data) = &sk.data.constraints[self.constraint] else {
            return;
        };
        let constraint = &mut sk.constraints[self.constraint];
        if !constraint.active {
            return;
        }
        let ConstraintBody::Ik(ik) = &mut constraint.body else {
            return;
        };
        let pose = ik.pose.target(o.applied_pose);
        let setup = data.setup_pose;
        let frames = &self.curve.frames;
        if time < frames[0] {
            match o.from {
                MixFrom::Setup => *pose = setup,
                MixFrom::First => {
                    pose.mix += (setup.mix - pose.mix) * o.alpha;
                    pose.softness += (setup.softness - pose.softness) * o.alpha;
                    pose.bend_direction = setup.bend_direction;
                    pose.compress = setup.compress;
                    pose.stretch = setup.stretch;
                }
                MixFrom::Current => {}
            }
            return;
        }
        let [mix, softness] = self.curve.sample_prefix::<2>(time);
        let i = super::search_step(frames, time, Self::ENTRIES);
        let base = if o.from == MixFrom::Setup {
            setup
        } else {
            *pose
        };
        pose.mix = base.mix + (mix - base.mix) * o.alpha;
        pose.softness = base.softness + (softness - base.softness) * o.alpha;
        if o.out {
            if o.from == MixFrom::Setup {
                pose.bend_direction = base.bend_direction;
                pose.compress = base.compress;
                pose.stretch = base.stretch;
            }
        } else {
            pose.bend_direction = frames[i + 3] as i32;
            pose.compress = frames[i + 4] != 0.0;
            pose.stretch = frames[i + 5] != 0.0;
        }
    }
}

/// Changes the mixes of a transform constraint.
#[derive(Clone, Debug)]
pub struct TransformConstraintTimeline {
    pub curve: CurveTimeline,
    pub constraint: usize,
}

impl TransformConstraintTimeline {
    const ENTRIES: usize = 7;

    pub fn new(frame_count: usize, bezier_count: usize, constraint: usize) -> Self {
        TransformConstraintTimeline {
            curve: CurveTimeline::new(frame_count, Self::ENTRIES, bezier_count),
            constraint,
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub fn set_frame(
        &mut self,
        frame: usize,
        time: f32,
        mix_rotate: f32,
        mix_x: f32,
        mix_y: f32,
        mix_scale_x: f32,
        mix_scale_y: f32,
        mix_shear_y: f32,
    ) {
        let i = frame * Self::ENTRIES;
        let f = &mut self.curve.frames;
        f[i] = time;
        f[i + 1] = mix_rotate;
        f[i + 2] = mix_x;
        f[i + 3] = mix_y;
        f[i + 4] = mix_scale_x;
        f[i + 5] = mix_scale_y;
        f[i + 6] = mix_shear_y;
    }

    pub(crate) fn property_ids(&self) -> SmallVec<[PropertyId; 3]> {
        smallvec![Property::TransformConstraint.id(self.constraint)]
    }

    pub(crate) fn apply(&self, sk: &mut Skeleton, time: f32, o: &ApplyOptions) {
        let ConstraintData::Transform(data) = &sk.data.constraints[self.constraint] else {
            return;
        };
        let constraint = &mut sk.constraints[self.constraint];
        if !constraint.active {
            return;
        }
        let ConstraintBody::Transform(tc) = &mut constraint.body else {
            return;
        };
        let pose = tc.pose.target(o.applied_pose);
        let setup = data.setup_pose;
        if time < self.curve.frames[0] {
            match o.from {
                MixFrom::Setup => *pose = setup,
                MixFrom::First => {
                    pose.mix_rotate += (setup.mix_rotate - pose.mix_rotate) * o.alpha;
                    pose.mix_x += (setup.mix_x - pose.mix_x) * o.alpha;
                    pose.mix_y += (setup.mix_y - pose.mix_y) * o.alpha;
                    pose.mix_scale_x += (setup.mix_scale_x - pose.mix_scale_x) * o.alpha;
                    pose.mix_scale_y += (setup.mix_scale_y - pose.mix_scale_y) * o.alpha;
                    pose.mix_shear_y += (setup.mix_shear_y - pose.mix_shear_y) * o.alpha;
                }
                MixFrom::Current => {}
            }
            return;
        }
        let [rotate, x, y, scale_x, scale_y, shear_y] = self.curve.sample::<6>(time);
        let base = if o.from == MixFrom::Setup {
            setup
        } else {
            *pose
        };
        let a = o.alpha;
        if o.add {
            pose.mix_rotate = base.mix_rotate + rotate * a;
            pose.mix_x = base.mix_x + x * a;
            pose.mix_y = base.mix_y + y * a;
            pose.mix_scale_x = base.mix_scale_x + scale_x * a;
            pose.mix_scale_y = base.mix_scale_y + scale_y * a;
            pose.mix_shear_y = base.mix_shear_y + shear_y * a;
        } else {
            pose.mix_rotate = base.mix_rotate + (rotate - base.mix_rotate) * a;
            pose.mix_x = base.mix_x + (x - base.mix_x) * a;
            pose.mix_y = base.mix_y + (y - base.mix_y) * a;
            pose.mix_scale_x = base.mix_scale_x + (scale_x - base.mix_scale_x) * a;
            pose.mix_scale_y = base.mix_scale_y + (scale_y - base.mix_scale_y) * a;
            pose.mix_shear_y = base.mix_shear_y + (shear_y - base.mix_shear_y) * a;
        }
    }
}

/// Changes the mixes of a path constraint.
#[derive(Clone, Debug)]
pub struct PathConstraintMixTimeline {
    pub curve: CurveTimeline,
    pub constraint: usize,
}

impl PathConstraintMixTimeline {
    const ENTRIES: usize = 4;

    pub fn new(frame_count: usize, bezier_count: usize, constraint: usize) -> Self {
        PathConstraintMixTimeline {
            curve: CurveTimeline::new(frame_count, Self::ENTRIES, bezier_count),
            constraint,
        }
    }

    pub fn set_frame(&mut self, frame: usize, time: f32, mix_rotate: f32, mix_x: f32, mix_y: f32) {
        let i = frame * Self::ENTRIES;
        let f = &mut self.curve.frames;
        f[i] = time;
        f[i + 1] = mix_rotate;
        f[i + 2] = mix_x;
        f[i + 3] = mix_y;
    }

    pub(crate) fn property_ids(&self) -> SmallVec<[PropertyId; 3]> {
        smallvec![Property::PathConstraintMix.id(self.constraint)]
    }

    pub(crate) fn apply(&self, sk: &mut Skeleton, time: f32, o: &ApplyOptions) {
        let ConstraintData::Path(data) = &sk.data.constraints[self.constraint] else {
            return;
        };
        let constraint = &mut sk.constraints[self.constraint];
        if !constraint.active {
            return;
        }
        let ConstraintBody::Path(pc) = &mut constraint.body else {
            return;
        };
        let pose = pc.pose.target(o.applied_pose);
        let setup = data.setup_pose;
        if time < self.curve.frames[0] {
            match o.from {
                MixFrom::Setup => {
                    pose.mix_rotate = setup.mix_rotate;
                    pose.mix_x = setup.mix_x;
                    pose.mix_y = setup.mix_y;
                }
                MixFrom::First => {
                    pose.mix_rotate += (setup.mix_rotate - pose.mix_rotate) * o.alpha;
                    pose.mix_x += (setup.mix_x - pose.mix_x) * o.alpha;
                    pose.mix_y += (setup.mix_y - pose.mix_y) * o.alpha;
                }
                MixFrom::Current => {}
            }
            return;
        }
        let [rotate, x, y] = self.curve.sample::<3>(time);
        let base = if o.from == MixFrom::Setup {
            setup
        } else {
            *pose
        };
        let a = o.alpha;
        if o.add {
            pose.mix_rotate = base.mix_rotate + rotate * a;
            pose.mix_x = base.mix_x + x * a;
            pose.mix_y = base.mix_y + y * a;
        } else {
            pose.mix_rotate = base.mix_rotate + (rotate - base.mix_rotate) * a;
            pose.mix_x = base.mix_x + (x - base.mix_x) * a;
            pose.mix_y = base.mix_y + (y - base.mix_y) * a;
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Constraint1Kind {
    PathPosition,
    PathSpacing,
    SliderTime,
    SliderMix,
    PhysicsInertia,
    PhysicsStrength,
    PhysicsDamping,
    PhysicsMass,
    PhysicsWind,
    PhysicsGravity,
    PhysicsMix,
}

impl Constraint1Kind {
    fn property(self) -> Property {
        match self {
            Constraint1Kind::PathPosition => Property::PathConstraintPosition,
            Constraint1Kind::PathSpacing => Property::PathConstraintSpacing,
            Constraint1Kind::SliderTime => Property::SliderTime,
            Constraint1Kind::SliderMix => Property::SliderMix,
            Constraint1Kind::PhysicsInertia => Property::PhysicsConstraintInertia,
            Constraint1Kind::PhysicsStrength => Property::PhysicsConstraintStrength,
            Constraint1Kind::PhysicsDamping => Property::PhysicsConstraintDamping,
            Constraint1Kind::PhysicsMass => Property::PhysicsConstraintMass,
            Constraint1Kind::PhysicsWind => Property::PhysicsConstraintWind,
            Constraint1Kind::PhysicsGravity => Property::PhysicsConstraintGravity,
            Constraint1Kind::PhysicsMix => Property::PhysicsConstraintMix,
        }
    }

    pub fn is_physics(self) -> bool {
        matches!(
            self,
            Constraint1Kind::PhysicsInertia
                | Constraint1Kind::PhysicsStrength
                | Constraint1Kind::PhysicsDamping
                | Constraint1Kind::PhysicsMass
                | Constraint1Kind::PhysicsWind
                | Constraint1Kind::PhysicsGravity
                | Constraint1Kind::PhysicsMix
        )
    }

    /// True for timelines that support additive blending.
    pub fn additive(self) -> bool {
        matches!(
            self,
            Constraint1Kind::PathPosition
                | Constraint1Kind::SliderMix
                | Constraint1Kind::PhysicsWind
                | Constraint1Kind::PhysicsGravity
        )
    }

    fn physics_get(self, pose: &PhysicsConstraintPose) -> f32 {
        match self {
            Constraint1Kind::PhysicsInertia => pose.inertia,
            Constraint1Kind::PhysicsStrength => pose.strength,
            Constraint1Kind::PhysicsDamping => pose.damping,
            Constraint1Kind::PhysicsMass => 1.0 / pose.mass_inverse,
            Constraint1Kind::PhysicsWind => pose.wind,
            Constraint1Kind::PhysicsGravity => pose.gravity,
            _ => pose.mix,
        }
    }

    fn physics_set(self, pose: &mut PhysicsConstraintPose, value: f32) {
        match self {
            Constraint1Kind::PhysicsInertia => pose.inertia = value,
            Constraint1Kind::PhysicsStrength => pose.strength = value,
            Constraint1Kind::PhysicsDamping => pose.damping = value,
            Constraint1Kind::PhysicsMass => pose.mass_inverse = 1.0 / value,
            Constraint1Kind::PhysicsWind => pose.wind = value,
            Constraint1Kind::PhysicsGravity => pose.gravity = value,
            _ => pose.mix = value,
        }
    }

    fn physics_global(self, d: &PhysicsConstraintData) -> bool {
        match self {
            Constraint1Kind::PhysicsInertia => d.inertia_global,
            Constraint1Kind::PhysicsStrength => d.strength_global,
            Constraint1Kind::PhysicsDamping => d.damping_global,
            Constraint1Kind::PhysicsMass => d.mass_global,
            Constraint1Kind::PhysicsWind => d.wind_global,
            Constraint1Kind::PhysicsGravity => d.gravity_global,
            _ => d.mix_global,
        }
    }
}

/// A timeline changing one value of a constraint: path position/spacing, slider time/mix, or a
/// physics setting.
#[derive(Clone, Debug)]
pub struct ConstraintTimeline1 {
    pub curve: CurveTimeline,
    /// The constraint index; `None` applies a physics timeline to every physics constraint with
    /// the setting marked global.
    pub constraint: Option<usize>,
    pub kind: Constraint1Kind,
}

impl ConstraintTimeline1 {
    pub fn new(
        kind: Constraint1Kind,
        frame_count: usize,
        bezier_count: usize,
        constraint: Option<usize>,
    ) -> Self {
        ConstraintTimeline1 {
            curve: CurveTimeline::new(frame_count, 2, bezier_count),
            constraint,
            kind,
        }
    }

    pub fn set_frame(&mut self, frame: usize, time: f32, value: f32) {
        self.curve.set_frame1(frame, time, value);
    }

    pub(crate) fn property_ids(&self) -> SmallVec<[PropertyId; 3]> {
        smallvec![constraint_property_id(
            self.kind.property(),
            self.constraint
        )]
    }

    pub(crate) fn apply(&self, sk: &mut Skeleton, time: f32, o: &ApplyOptions) {
        use Constraint1Kind as K;
        let c = &self.curve;
        let (alpha, from) = (o.alpha, o.from);
        let mut add = o.add;
        if self.kind.is_physics() {
            if add && !self.kind.additive() {
                add = false;
            }
            match self.constraint {
                None => {
                    let value = if time >= c.frames[0] {
                        c.curve_value(time)
                    } else {
                        0.0
                    };
                    for i in 0..sk.constraints.len() {
                        let ConstraintData::Physics(data) = &sk.data.constraints[i] else {
                            continue;
                        };
                        let constraint = &mut sk.constraints[i];
                        if !constraint.active || !self.kind.physics_global(data) {
                            continue;
                        }
                        if let ConstraintBody::Physics(pc) = &mut constraint.body {
                            let pose = pc.pose.target(o.applied_pose);
                            let v = c.absolute_value_of(
                                time,
                                alpha,
                                from,
                                add,
                                self.kind.physics_get(pose),
                                self.kind.physics_get(&data.setup_pose),
                                value,
                            );
                            self.kind.physics_set(pose, v);
                        }
                    }
                }
                Some(i) => {
                    let ConstraintData::Physics(data) = &sk.data.constraints[i] else {
                        return;
                    };
                    let constraint = &mut sk.constraints[i];
                    if !constraint.active {
                        return;
                    }
                    if let ConstraintBody::Physics(pc) = &mut constraint.body {
                        let pose = pc.pose.target(o.applied_pose);
                        let v = c.absolute_value(
                            time,
                            alpha,
                            from,
                            add,
                            self.kind.physics_get(pose),
                            self.kind.physics_get(&data.setup_pose),
                        );
                        self.kind.physics_set(pose, v);
                    }
                }
            }
            return;
        }

        let Some(i) = self.constraint else {
            return;
        };
        let constraint = &mut sk.constraints[i];
        if !constraint.active {
            return;
        }
        match (self.kind, &mut constraint.body, &sk.data.constraints[i]) {
            (K::PathPosition, ConstraintBody::Path(pc), ConstraintData::Path(d)) => {
                let pose = pc.pose.target(o.applied_pose);
                pose.position =
                    c.absolute_value(time, alpha, from, add, pose.position, d.setup_pose.position);
            }
            (K::PathSpacing, ConstraintBody::Path(pc), ConstraintData::Path(d)) => {
                let pose = pc.pose.target(o.applied_pose);
                pose.spacing =
                    c.absolute_value(time, alpha, from, false, pose.spacing, d.setup_pose.spacing);
            }
            (K::SliderTime, ConstraintBody::Slider(s), ConstraintData::Slider(d)) => {
                let pose = s.pose.target(o.applied_pose);
                pose.time = c.absolute_value(time, alpha, from, add, pose.time, d.setup_pose.time);
            }
            (K::SliderMix, ConstraintBody::Slider(s), ConstraintData::Slider(d)) => {
                let pose = s.pose.target(o.applied_pose);
                pose.mix = c.absolute_value(time, alpha, from, add, pose.mix, d.setup_pose.mix);
            }
            _ => {}
        }
    }
}

/// Resets physics constraints at discrete times.
#[derive(Clone, Debug)]
pub struct PhysicsConstraintResetTimeline {
    pub frames: Vec<f32>,
    /// The constraint index, or `None` for all physics constraints.
    pub constraint: Option<usize>,
}

impl PhysicsConstraintResetTimeline {
    pub fn new(frame_count: usize, constraint: Option<usize>) -> Self {
        PhysicsConstraintResetTimeline {
            frames: vec![0.0; frame_count],
            constraint,
        }
    }

    pub fn set_frame(&mut self, frame: usize, time: f32) {
        self.frames[frame] = time;
    }

    pub(crate) fn property_ids(&self) -> SmallVec<[PropertyId; 3]> {
        smallvec![(Property::PhysicsConstraintReset as u64) << 32]
    }

    pub(crate) fn apply(&self, sk: &mut Skeleton, last_time: f32, time: f32) {
        if let Some(i) = self.constraint {
            if !sk.constraints[i].active {
                return;
            }
        }
        let frames = &self.frames;
        let mut last = last_time;
        if last > time {
            // Looped: apply after last_time up to the end, then from the start.
            self.reset_in_range(sk, last, f32::MAX);
            last = -1.0;
        } else if last >= frames[frames.len() - 1] {
            return;
        }
        if time < frames[0] {
            return;
        }
        if last < frames[0] || time >= frames[search(frames, last) + 1] {
            self.reset(sk);
        }
    }

    fn reset_in_range(&self, sk: &mut Skeleton, last: f32, time: f32) {
        let frames = &self.frames;
        if last >= frames[frames.len() - 1] || time < frames[0] {
            return;
        }
        if last < frames[0] || time >= frames[search(frames, last) + 1] {
            self.reset(sk);
        }
    }

    fn reset(&self, sk: &mut Skeleton) {
        let skeleton_time = sk.time;
        match self.constraint {
            Some(i) => {
                if let ConstraintBody::Physics(p) = &mut sk.constraints[i].body {
                    p.reset(skeleton_time);
                }
            }
            None => {
                for c in &mut sk.constraints {
                    if c.active {
                        if let ConstraintBody::Physics(p) = &mut c.body {
                            p.reset(skeleton_time);
                        }
                    }
                }
            }
        }
    }
}
