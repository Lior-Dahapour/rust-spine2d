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
use super::{ApplyOptions, MixFrom, Property, PropertyId};
use crate::color::Color;
use crate::skeleton::Skeleton;
use crate::skeleton_data::SkeletonData;
use crate::slot::SlotPose;
use smallvec::{smallvec, SmallVec};

#[inline]
fn clamp01(v: f32) -> f32 {
    v.clamp(0.0, 1.0)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ColorKind {
    /// Light color with alpha.
    Rgba,
    /// Light color without alpha.
    Rgb,
    /// Light color with alpha, and dark color.
    Rgba2,
    /// Light and dark color, without alpha.
    Rgb2,
}

impl ColorKind {
    /// Floats per frame, including the time.
    pub const fn entries(self) -> usize {
        match self {
            ColorKind::Rgba => 5,
            ColorKind::Rgb => 4,
            ColorKind::Rgba2 => 8,
            ColorKind::Rgb2 => 7,
        }
    }
}

/// A timeline that changes a slot's light (and optionally dark) color.
#[derive(Clone, Debug)]
pub struct ColorTimeline {
    pub curve: CurveTimeline,
    pub slot: usize,
    pub kind: ColorKind,
}

impl ColorTimeline {
    pub fn new(kind: ColorKind, frame_count: usize, bezier_count: usize, slot: usize) -> Self {
        ColorTimeline {
            curve: CurveTimeline::new(frame_count, kind.entries(), bezier_count),
            slot,
            kind,
        }
    }

    /// Sets a frame; `values` are the keyed color components in the order of the kind:
    /// rgba: r g b a; rgb: r g b; rgba2: r g b a r2 g2 b2; rgb2: r g b r2 g2 b2.
    pub fn set_frame(&mut self, frame: usize, time: f32, values: &[f32]) {
        debug_assert_eq!(values.len() + 1, self.kind.entries());
        let i = frame * self.kind.entries();
        self.curve.frames[i] = time;
        self.curve.frames[i + 1..i + 1 + values.len()].copy_from_slice(values);
    }

    pub(crate) fn property_ids(&self) -> SmallVec<[PropertyId; 3]> {
        let s = self.slot;
        match self.kind {
            ColorKind::Rgba => smallvec![Property::Rgb.id(s), Property::Alpha.id(s)],
            ColorKind::Rgb => smallvec![Property::Rgb.id(s)],
            ColorKind::Rgba2 => smallvec![
                Property::Rgb.id(s),
                Property::Alpha.id(s),
                Property::Rgb2.id(s)
            ],
            ColorKind::Rgb2 => smallvec![Property::Rgb.id(s), Property::Rgb2.id(s)],
        }
    }

    pub(crate) fn apply(
        &self,
        skeleton: &mut Skeleton,
        skeleton_data: &SkeletonData,
        time: f32,
        o: &ApplyOptions,
    ) {
        let slot = &mut skeleton.slots[self.slot];
        if !skeleton.bones[slot.bone.0].active {
            return;
        }
        let setup = &skeleton_data.slots[self.slot].setup_pose;
        let pose = if o.applied_pose {
            slot.applied_pose_mut()
        } else {
            &mut slot.pose
        };
        match self.kind {
            ColorKind::Rgba => self.apply_rgba(pose, setup, time, o),
            ColorKind::Rgb => self.apply_rgb(pose, setup, time, o),
            ColorKind::Rgba2 => self.apply_rgba2(pose, setup, time, o),
            ColorKind::Rgb2 => self.apply_rgb2(pose, setup, time, o),
        }
    }

    fn apply_rgba(&self, pose: &mut SlotPose, setup: &SlotPose, time: f32, o: &ApplyOptions) {
        let color = &mut pose.color;
        if time < self.curve.frames[0] {
            let s = setup.color;
            match o.from {
                MixFrom::Setup => *color = s,
                MixFrom::First => {
                    color.add(
                        (s.r - color.r) * o.alpha,
                        (s.g - color.g) * o.alpha,
                        (s.b - color.b) * o.alpha,
                        (s.a - color.a) * o.alpha,
                    );
                }
                MixFrom::Current => {}
            }
            return;
        }
        let [r, g, b, a] = self.curve.sample::<4>(time);
        if o.alpha == 1.0 {
            color.set(r, g, b, a);
        } else if o.from == MixFrom::Setup {
            let s = setup.color;
            color.set(
                s.r + (r - s.r) * o.alpha,
                s.g + (g - s.g) * o.alpha,
                s.b + (b - s.b) * o.alpha,
                s.a + (a - s.a) * o.alpha,
            );
        } else {
            color.add(
                (r - color.r) * o.alpha,
                (g - color.g) * o.alpha,
                (b - color.b) * o.alpha,
                (a - color.a) * o.alpha,
            );
        }
    }

    fn apply_rgb(&self, pose: &mut SlotPose, setup: &SlotPose, time: f32, o: &ApplyOptions) {
        let color = &mut pose.color;
        if time < self.curve.frames[0] {
            let s = setup.color;
            match o.from {
                MixFrom::Setup => {
                    color.r = s.r;
                    color.g = s.g;
                    color.b = s.b;
                }
                MixFrom::First => {
                    color.r += (s.r - color.r) * o.alpha;
                    color.g += (s.g - color.g) * o.alpha;
                    color.b += (s.b - color.b) * o.alpha;
                }
                MixFrom::Current => {}
            }
            return;
        }
        let [mut r, mut g, mut b] = self.curve.sample::<3>(time);
        if o.alpha != 1.0 {
            let base = if o.from == MixFrom::Setup {
                setup.color
            } else {
                *color
            };
            r = base.r + (r - base.r) * o.alpha;
            g = base.g + (g - base.g) * o.alpha;
            b = base.b + (b - base.b) * o.alpha;
        }
        color.r = clamp01(r);
        color.g = clamp01(g);
        color.b = clamp01(b);
    }

    fn apply_rgba2(&self, pose: &mut SlotPose, setup: &SlotPose, time: f32, o: &ApplyOptions) {
        let (light, dark) = (&mut pose.color, &mut pose.dark_color);
        if time < self.curve.frames[0] {
            let (sl, sd) = (setup.color, setup.dark_color);
            match o.from {
                MixFrom::Setup => {
                    *light = sl;
                    dark.r = sd.r;
                    dark.g = sd.g;
                    dark.b = sd.b;
                }
                MixFrom::First => {
                    light.add(
                        (sl.r - light.r) * o.alpha,
                        (sl.g - light.g) * o.alpha,
                        (sl.b - light.b) * o.alpha,
                        (sl.a - light.a) * o.alpha,
                    );
                    dark.r += (sd.r - dark.r) * o.alpha;
                    dark.g += (sd.g - dark.g) * o.alpha;
                    dark.b += (sd.b - dark.b) * o.alpha;
                }
                MixFrom::Current => {}
            }
            return;
        }
        let [r, g, b, a, mut r2, mut g2, mut b2] = self.curve.sample::<7>(time);
        if o.alpha == 1.0 {
            light.set(r, g, b, a);
        } else if o.from == MixFrom::Setup {
            let (sl, sd) = (setup.color, setup.dark_color);
            light.set(
                sl.r + (r - sl.r) * o.alpha,
                sl.g + (g - sl.g) * o.alpha,
                sl.b + (b - sl.b) * o.alpha,
                sl.a + (a - sl.a) * o.alpha,
            );
            r2 = sd.r + (r2 - sd.r) * o.alpha;
            g2 = sd.g + (g2 - sd.g) * o.alpha;
            b2 = sd.b + (b2 - sd.b) * o.alpha;
        } else {
            light.add(
                (r - light.r) * o.alpha,
                (g - light.g) * o.alpha,
                (b - light.b) * o.alpha,
                (a - light.a) * o.alpha,
            );
            r2 = dark.r + (r2 - dark.r) * o.alpha;
            g2 = dark.g + (g2 - dark.g) * o.alpha;
            b2 = dark.b + (b2 - dark.b) * o.alpha;
        }
        dark.r = clamp01(r2);
        dark.g = clamp01(g2);
        dark.b = clamp01(b2);
    }

    fn apply_rgb2(&self, pose: &mut SlotPose, setup: &SlotPose, time: f32, o: &ApplyOptions) {
        let (light, dark) = (&mut pose.color, &mut pose.dark_color);
        if time < self.curve.frames[0] {
            let (sl, sd) = (setup.color, setup.dark_color);
            match o.from {
                MixFrom::Setup => {
                    light.r = sl.r;
                    light.g = sl.g;
                    light.b = sl.b;
                    dark.r = sd.r;
                    dark.g = sd.g;
                    dark.b = sd.b;
                }
                MixFrom::First => {
                    light.r += (sl.r - light.r) * o.alpha;
                    light.g += (sl.g - light.g) * o.alpha;
                    light.b += (sl.b - light.b) * o.alpha;
                    dark.r += (sd.r - dark.r) * o.alpha;
                    dark.g += (sd.g - dark.g) * o.alpha;
                    dark.b += (sd.b - dark.b) * o.alpha;
                }
                MixFrom::Current => {}
            }
            return;
        }
        let [mut r, mut g, mut b, mut r2, mut g2, mut b2] = self.curve.sample::<6>(time);
        if o.alpha != 1.0 {
            let (bl, bd): (Color, Color) = if o.from == MixFrom::Setup {
                (setup.color, setup.dark_color)
            } else {
                (*light, *dark)
            };
            r = bl.r + (r - bl.r) * o.alpha;
            g = bl.g + (g - bl.g) * o.alpha;
            b = bl.b + (b - bl.b) * o.alpha;
            r2 = bd.r + (r2 - bd.r) * o.alpha;
            g2 = bd.g + (g2 - bd.g) * o.alpha;
            b2 = bd.b + (b2 - bd.b) * o.alpha;
        }
        light.r = clamp01(r);
        light.g = clamp01(g);
        light.b = clamp01(b);
        dark.r = clamp01(r2);
        dark.g = clamp01(g2);
        dark.b = clamp01(b2);
    }
}

/// A timeline that changes a slot's alpha.
#[derive(Clone, Debug)]
pub struct AlphaTimeline {
    pub curve: CurveTimeline,
    pub slot: usize,
}

impl AlphaTimeline {
    pub fn new(frame_count: usize, bezier_count: usize, slot: usize) -> Self {
        AlphaTimeline {
            curve: CurveTimeline::new(frame_count, 2, bezier_count),
            slot,
        }
    }

    pub fn set_frame(&mut self, frame: usize, time: f32, alpha: f32) {
        self.curve.set_frame1(frame, time, alpha);
    }

    pub(crate) fn property_ids(&self) -> SmallVec<[PropertyId; 3]> {
        smallvec![Property::Alpha.id(self.slot)]
    }

    pub(crate) fn apply(
        &self,
        skeleton: &mut Skeleton,
        skeleton_data: &SkeletonData,
        time: f32,
        o: &ApplyOptions,
    ) {
        let slot = &mut skeleton.slots[self.slot];
        if !skeleton.bones[slot.bone.0].active {
            return;
        }
        let setup_a = skeleton_data.slots[self.slot].setup_pose.color.a;
        let color = &mut (if o.applied_pose {
            slot.applied_pose_mut()
        } else {
            &mut slot.pose
        })
        .color;
        if time < self.curve.frames[0] {
            match o.from {
                MixFrom::Setup => color.a = setup_a,
                MixFrom::First => color.a += (setup_a - color.a) * o.alpha,
                MixFrom::Current => {}
            }
            return;
        }
        let mut a = self.curve.curve_value(time);
        if o.alpha != 1.0 {
            let base = if o.from == MixFrom::Setup {
                setup_a
            } else {
                color.a
            };
            a = base + (a - base) * o.alpha;
        }
        color.a = clamp01(a);
    }
}
