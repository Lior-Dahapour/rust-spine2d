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

use super::curve::{before_first_key, scale_blend, CurveTimeline};
use super::{search_step, ApplyOptions, MixFrom, Property, PropertyId};
use crate::bone::BoneLocal;
use crate::enums::Inherit;
use crate::skeleton::Skeleton;
use smallvec::{smallvec, SmallVec};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Bone1Kind {
    Rotate,
    X,
    Y,
    ScaleX,
    ScaleY,
    ShearX,
    ShearY,
}

impl Bone1Kind {
    fn property(self) -> Property {
        match self {
            Bone1Kind::Rotate => Property::Rotate,
            Bone1Kind::X => Property::X,
            Bone1Kind::Y => Property::Y,
            Bone1Kind::ScaleX => Property::ScaleX,
            Bone1Kind::ScaleY => Property::ScaleY,
            Bone1Kind::ShearX => Property::ShearX,
            Bone1Kind::ShearY => Property::ShearY,
        }
    }
}

/// A timeline that changes one value of a bone's local pose.
#[derive(Clone, Debug)]
pub struct BoneTimeline1 {
    pub curve: CurveTimeline,
    pub bone: usize,
    pub kind: Bone1Kind,
}

impl BoneTimeline1 {
    pub fn new(kind: Bone1Kind, frame_count: usize, bezier_count: usize, bone: usize) -> Self {
        BoneTimeline1 {
            curve: CurveTimeline::new(frame_count, 2, bezier_count),
            bone,
            kind,
        }
    }

    pub fn set_frame(&mut self, frame: usize, time: f32, value: f32) {
        self.curve.set_frame1(frame, time, value);
    }

    pub(crate) fn property_ids(&self) -> SmallVec<[PropertyId; 3]> {
        smallvec![self.kind.property().id(self.bone)]
    }

    pub(crate) fn apply(&self, skeleton: &mut Skeleton, time: f32, o: &ApplyOptions) {
        let bone = &mut skeleton.bones[self.bone];
        if !bone.active {
            return;
        }
        let setup = &skeleton.data.bones[self.bone].setup_pose;
        let local = if o.applied_pose {
            &mut bone.applied_pose_mut().local
        } else {
            &mut bone.pose.local
        };
        self.apply_local(local, setup, time, o);
    }

    fn apply_local(&self, l: &mut BoneLocal, s: &BoneLocal, time: f32, o: &ApplyOptions) {
        let c = &self.curve;
        let (alpha, from, add, out) = (o.alpha, o.from, o.add, o.out);
        match self.kind {
            Bone1Kind::Rotate => {
                l.rotation = c.relative_value(time, alpha, from, add, l.rotation, s.rotation)
            }
            Bone1Kind::X => l.x = c.relative_value(time, alpha, from, add, l.x, s.x),
            Bone1Kind::Y => l.y = c.relative_value(time, alpha, from, add, l.y, s.y),
            Bone1Kind::ScaleX => {
                l.scale_x = c.scale_value(time, alpha, from, add, out, l.scale_x, s.scale_x)
            }
            Bone1Kind::ScaleY => {
                l.scale_y = c.scale_value(time, alpha, from, add, out, l.scale_y, s.scale_y)
            }
            Bone1Kind::ShearX => {
                l.shear_x = c.relative_value(time, alpha, from, add, l.shear_x, s.shear_x)
            }
            Bone1Kind::ShearY => {
                l.shear_y = c.relative_value(time, alpha, from, add, l.shear_y, s.shear_y)
            }
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Bone2Kind {
    /// x and y.
    Translate,
    /// scale x and y.
    Scale,
    /// shear x and y.
    Shear,
}

/// A timeline that changes two values (x/y pairs) of a bone's local pose.
#[derive(Clone, Debug)]
pub struct BoneTimeline2 {
    pub curve: CurveTimeline,
    pub bone: usize,
    pub kind: Bone2Kind,
}

impl BoneTimeline2 {
    pub fn new(kind: Bone2Kind, frame_count: usize, bezier_count: usize, bone: usize) -> Self {
        BoneTimeline2 {
            curve: CurveTimeline::new(frame_count, 3, bezier_count),
            bone,
            kind,
        }
    }

    pub fn set_frame(&mut self, frame: usize, time: f32, value1: f32, value2: f32) {
        let i = frame * 3;
        self.curve.frames[i] = time;
        self.curve.frames[i + 1] = value1;
        self.curve.frames[i + 2] = value2;
    }

    pub(crate) fn property_ids(&self) -> SmallVec<[PropertyId; 3]> {
        let (a, b) = match self.kind {
            Bone2Kind::Translate => (Property::X, Property::Y),
            Bone2Kind::Scale => (Property::ScaleX, Property::ScaleY),
            Bone2Kind::Shear => (Property::ShearX, Property::ShearY),
        };
        smallvec![a.id(self.bone), b.id(self.bone)]
    }

    pub(crate) fn apply(&self, skeleton: &mut Skeleton, time: f32, o: &ApplyOptions) {
        let bone = &mut skeleton.bones[self.bone];
        if !bone.active {
            return;
        }
        let setup = &skeleton.data.bones[self.bone].setup_pose;
        let local = if o.applied_pose {
            &mut bone.applied_pose_mut().local
        } else {
            &mut bone.pose.local
        };
        match self.kind {
            Bone2Kind::Translate => {
                let (x, y) = self.apply_pair(time, o, (local.x, local.y), (setup.x, setup.y));
                local.x = x;
                local.y = y;
            }
            Bone2Kind::Shear => {
                let (x, y) = self.apply_pair(
                    time,
                    o,
                    (local.shear_x, local.shear_y),
                    (setup.shear_x, setup.shear_y),
                );
                local.shear_x = x;
                local.shear_y = y;
            }
            Bone2Kind::Scale => {
                let (x, y) = self.apply_scale(
                    time,
                    o,
                    (local.scale_x, local.scale_y),
                    (setup.scale_x, setup.scale_y),
                );
                local.scale_x = x;
                local.scale_y = y;
            }
        }
    }

    /// Translate/shear blending: the keyed values are offsets from setup.
    #[inline]
    fn apply_pair(
        &self,
        time: f32,
        o: &ApplyOptions,
        current: (f32, f32),
        setup: (f32, f32),
    ) -> (f32, f32) {
        let (mut px, mut py) = current;
        if time < self.curve.frames[0] {
            match o.from {
                MixFrom::Setup => {
                    px = setup.0;
                    py = setup.1;
                }
                MixFrom::First => {
                    px += (setup.0 - px) * o.alpha;
                    py += (setup.1 - py) * o.alpha;
                }
                MixFrom::Current => {}
            }
            return (px, py);
        }
        let [x, y] = self.curve.sample::<2>(time);
        if o.from == MixFrom::Setup {
            (setup.0 + x * o.alpha, setup.1 + y * o.alpha)
        } else if o.add {
            (px + x * o.alpha, py + y * o.alpha)
        } else {
            (
                px + (setup.0 + x - px) * o.alpha,
                py + (setup.1 + y - py) * o.alpha,
            )
        }
    }

    #[inline]
    fn apply_scale(
        &self,
        time: f32,
        o: &ApplyOptions,
        current: (f32, f32),
        setup: (f32, f32),
    ) -> (f32, f32) {
        if time < self.curve.frames[0] {
            return (
                before_first_key(o.from, o.alpha, current.0, setup.0),
                before_first_key(o.from, o.alpha, current.1, setup.1),
            );
        }
        let [x, y] = self.curve.sample::<2>(time);
        let (x, y) = (x * setup.0, y * setup.1);
        (
            scale_blend(x, o.alpha, o.from, o.add, o.out, current.0, setup.0),
            scale_blend(y, o.alpha, o.from, o.add, o.out, current.1, setup.1),
        )
    }
}

/// Sets a bone's [`Inherit`] mode at discrete times.
#[derive(Clone, Debug)]
pub struct InheritTimeline {
    pub frames: Vec<f32>,
    pub bone: usize,
}

impl InheritTimeline {
    const ENTRIES: usize = 2;

    pub fn new(frame_count: usize, bone: usize) -> Self {
        InheritTimeline {
            frames: vec![0.0; frame_count * Self::ENTRIES],
            bone,
        }
    }

    pub fn set_frame(&mut self, frame: usize, time: f32, inherit: Inherit) {
        let i = frame * Self::ENTRIES;
        self.frames[i] = time;
        self.frames[i + 1] = inherit as usize as f32;
    }

    pub(crate) fn property_ids(&self) -> SmallVec<[PropertyId; 3]> {
        smallvec![Property::Inherit.id(self.bone)]
    }

    pub(crate) fn apply(&self, skeleton: &mut Skeleton, time: f32, o: &ApplyOptions) {
        let bone = &mut skeleton.bones[self.bone];
        if !bone.active {
            return;
        }
        let setup = skeleton.data.bones[self.bone].setup_pose.inherit;
        let local = if o.applied_pose {
            &mut bone.applied_pose_mut().local
        } else {
            &mut bone.pose.local
        };
        if o.out || time < self.frames[0] {
            if o.from != MixFrom::Current {
                local.inherit = setup;
            }
        } else {
            let idx = search_step(&self.frames, time, Self::ENTRIES) + 1;
            local.inherit = Inherit::from_index(self.frames[idx] as usize);
        }
    }
}
