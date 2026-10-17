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

use super::{Constraint, ConstraintBody, PoseSet};
use crate::bone::{is_y_down, BoneId};
use crate::constraint_data::{
    ConstraintData, PhysicsConstraintData, PhysicsConstraintPose, ScaleYMode,
};
use crate::enums::Physics;
use crate::math::{self, abs, DEG_RAD, INV_PI_2, PI_2};
use crate::skeleton::{Skeleton, Update};
use crate::skeleton_data::SkeletonData;

/// The simulation state of a physics constraint.
#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct State {
    reset: bool,
    ux: f32,
    uy: f32,
    cx: f32,
    cy: f32,
    tx: f32,
    ty: f32,
    x_offset: f32,
    x_lag: f32,
    x_velocity: f32,
    y_offset: f32,
    y_lag: f32,
    y_velocity: f32,
    rotate_offset: f32,
    rotate_lag: f32,
    rotate_velocity: f32,
    scale_offset: f32,
    scale_lag: f32,
    scale_velocity: f32,
    remaining: f32,
    last_time: f32,
}

impl State {
    fn new() -> State {
        State {
            reset: true,
            ..Default::default()
        }
    }

    fn reset(&mut self, time: f32) {
        *self = State {
            ux: self.ux,
            uy: self.uy,
            cx: self.cx,
            cy: self.cy,
            tx: self.tx,
            ty: self.ty,
            last_time: time,
            reset: true,
            ..Default::default()
        };
    }

    fn translate(&mut self, x: f32, y: f32) {
        self.ux -= x;
        self.uy -= y;
        self.cx -= x;
        self.cy -= y;
    }

    fn rotate(&mut self, x: f32, y: f32, degrees: f32) {
        let r = degrees * DEG_RAD;
        let (cos_val, sin_val) = (math::cos(r), math::sin(r));
        let dx = self.cx - x;
        let dy = self.cy - y;
        self.translate(
            dx * cos_val - dy * sin_val - dx,
            dx * sin_val + dy * cos_val - dy,
        );
    }
}

/// Simulates a spring-like response of a bone to skeleton movement, wind and gravity.
#[derive(Clone, Debug)]
pub struct PhysicsConstraint {
    pub pose: PoseSet<PhysicsConstraintPose>,
    pub(crate) bone: BoneId,
    state: State,
}

impl PhysicsConstraint {
    pub(crate) fn new(d: &PhysicsConstraintData) -> Self {
        PhysicsConstraint {
            pose: PoseSet::new(d.setup_pose),
            bone: BoneId(d.bone),
            state: State::new(),
        }
    }

    /// The bone the simulation moves.
    pub fn bone(&self) -> BoneId {
        self.bone
    }

    /// Makes the simulation move `bone`. Call [`update_cache`](crate::skeleton::Borrowed::update_cache) afterwards.
    pub fn set_bone(&mut self, bone: BoneId) {
        self.bone = bone;
    }

    /// Resets the simulation to the current pose.
    pub fn reset(&mut self, skeleton_time: f32) {
        self.state.reset(skeleton_time);
    }

    /// Moves the simulation as if the skeleton moved by `x`, `y` (so physics don't react to
    /// the move).
    pub fn translate(&mut self, x: f32, y: f32) {
        self.state.translate(x, y);
    }

    /// Rotates the simulation around `x`, `y` as if the skeleton rotated.
    pub fn rotate(&mut self, x: f32, y: f32, degrees: f32) {
        self.state.rotate(x, y, degrees);
    }
}

fn parts(c: &Constraint) -> &PhysicsConstraint {
    match &c.body {
        ConstraintBody::Physics(p) => p,
        _ => unreachable!("not a physics constraint"),
    }
}

pub(super) fn sort(sk: &mut Skeleton, index: usize) {
    let bone = parts(&sk.constraints[index]).bone;
    sk.sort_bone(bone);
    sk.update_cache.push(Update::Constraint(index));
    sk.sort_reset_children(bone);
    sk.constrained_bone(bone);
}

pub(super) fn update(sk: &mut Skeleton, data: &SkeletonData, index: usize, physics: Physics) {
    let ConstraintData::Physics(d) = &data.constraints[index] else {
        return;
    };
    let pc = parts(&sk.constraints[index]);
    let p = *pc.pose.applied();
    let mix = p.mix;
    if mix == 0.0 {
        return;
    }
    let (bone_id, mut st) = (pc.bone, pc.state);
    let x = d.x > 0.0;
    let y = d.y > 0.0;
    let rotate_or_shear_x = d.rotate > 0.0 || d.shear_x > 0.0;
    let scale_x = d.scale_x > 0.0;
    let l = data.bones[bone_id.0].length;
    let t = d.step;
    let mut z = 0.0f32;
    if physics == Physics::None {
        return;
    }
    sk.modify_bone_world(bone_id);
    let mut bone = *sk.bones[bone_id.0].applied_pose();
    let (skeleton_time, scale_x_sk, scale_y_sk) = (sk.time, sk.scale_x, sk.scale_y());

    match physics {
        Physics::Reset | Physics::Update => {
            if physics == Physics::Reset {
                st.reset(skeleton_time);
            }
            let delta = (skeleton_time - st.last_time).max(0.0);
            let aa = st.remaining;
            st.remaining += delta;
            st.last_time = skeleton_time;
            let bx = bone.world_x;
            let by = bone.world_y;
            if st.reset {
                st.reset = false;
                st.ux = bx;
                st.uy = by;
            } else {
                let mut a = st.remaining;
                let i = p.inertia;
                let f = data.reference_scale;
                let mut dmp = -1.0f32;
                let mut m = 0.0f32;
                let mut e = 0.0f32;
                let mut qx = d.limit * delta;
                let qy = qx * abs(scale_y_sk);
                qx *= abs(sk.scale_x);
                if x || y {
                    if x {
                        let u = (st.ux - bx) * i;
                        st.x_offset += if u > qx {
                            qx
                        } else if u < -qx {
                            -qx
                        } else {
                            u
                        };
                        st.ux = bx;
                    }
                    if y {
                        let u = (st.uy - by) * i;
                        st.y_offset += if u > qy {
                            qy
                        } else if u < -qy {
                            -qy
                        } else {
                            u
                        };
                        st.uy = by;
                    }
                    if a >= t {
                        let xs = st.x_offset;
                        let ys = st.y_offset;
                        dmp = math::pow(p.damping, 60.0 * t);
                        m = t * p.mass_inverse;
                        e = p.strength;
                        let w = f * p.wind;
                        let g = f * p.gravity;
                        let ax = (w * sk.wind_x + g * sk.gravity_x) * scale_x_sk;
                        let ay = (w * sk.wind_y + g * sk.gravity_y) * scale_y_sk;
                        loop {
                            if x {
                                st.x_velocity += (ax - st.x_offset * e) * m;
                                st.x_offset += st.x_velocity * t;
                                st.x_velocity *= dmp;
                            }
                            if y {
                                st.y_velocity -= (ay + st.y_offset * e) * m;
                                st.y_offset += st.y_velocity * t;
                                st.y_velocity *= dmp;
                            }
                            a -= t;
                            if a < t {
                                break;
                            }
                        }
                        st.x_lag = st.x_offset - xs;
                        st.y_lag = st.y_offset - ys;
                    }
                    z = (0.0f32).max(1.0 - a / t);
                    if x {
                        bone.world_x += (st.x_offset - st.x_lag * z) * mix * d.x;
                    }
                    if y {
                        bone.world_y += (st.y_offset - st.y_lag * z) * mix * d.y;
                    }
                }
                if rotate_or_shear_x || scale_x {
                    let ca = math::atan2(bone.c, bone.a);
                    let (mut c, mut s);
                    let mut mr = 0.0f32;
                    let mut dx = st.cx - bone.world_x;
                    let mut dy = st.cy - bone.world_y;
                    if dx > qx {
                        dx = qx;
                    } else if dx < -qx {
                        dx = -qx;
                    }
                    if dy > qy {
                        dy = qy;
                    } else if dy < -qy {
                        dy = -qy;
                    }
                    if rotate_or_shear_x {
                        mr = (d.rotate + d.shear_x) * mix;
                        z = st.rotate_lag * (0.0f32).max(1.0 - aa / t);
                        let mut r =
                            math::atan2(dy + st.ty, dx + st.tx) - ca - (st.rotate_offset - z) * mr;
                        st.rotate_offset += (r - math::ceil(r * INV_PI_2 - 0.5) * PI_2) * i;
                        r = (st.rotate_offset - z) * mr + ca;
                        c = math::cos(r);
                        s = math::sin(r);
                        if scale_x {
                            r = l * bone.world_scale_x();
                            if r > 0.0 {
                                st.scale_offset += (dx * c + dy * s) * i / r;
                            }
                        }
                    } else {
                        c = math::cos(ca);
                        s = math::sin(ca);
                        let r =
                            l * bone.world_scale_x() - st.scale_lag * (0.0f32).max(1.0 - aa / t);
                        if r > 0.0 {
                            st.scale_offset += (dx * c + dy * s) * i / r;
                        }
                    }
                    a = st.remaining;
                    if a >= t {
                        if dmp == -1.0 {
                            dmp = math::pow(p.damping, 60.0 * t);
                            m = t * p.mass_inverse;
                            e = p.strength;
                        }
                        let ax = p.wind * sk.wind_x + p.gravity * sk.gravity_x;
                        let ay = (p.wind * sk.wind_y + p.gravity * sk.gravity_y)
                            * if is_y_down() { -1.0 } else { 1.0 };
                        let rs = st.rotate_offset;
                        let ss = st.scale_offset;
                        let h = l / f;
                        loop {
                            a -= t;
                            if scale_x {
                                st.scale_velocity += (ax * c - ay * s - st.scale_offset * e) * m;
                                st.scale_offset += st.scale_velocity * t;
                                st.scale_velocity *= dmp;
                            }
                            if rotate_or_shear_x {
                                st.rotate_velocity -=
                                    ((ax * s + ay * c) * h + st.rotate_offset * e) * m;
                                st.rotate_offset += st.rotate_velocity * t;
                                st.rotate_velocity *= dmp;
                                if a < t {
                                    break;
                                }
                                let r = st.rotate_offset * mr + ca;
                                c = math::cos(r);
                                s = math::sin(r);
                            } else if a < t {
                                break;
                            }
                        }
                        st.rotate_lag = st.rotate_offset - rs;
                        st.scale_lag = st.scale_offset - ss;
                    }
                    z = (0.0f32).max(1.0 - a / t);
                }
                st.remaining = a;
            }
            st.cx = bone.world_x;
            st.cy = bone.world_y;
        }
        Physics::Pose => {
            z = (0.0f32).max(1.0 - st.remaining / t);
            if x {
                bone.world_x += (st.x_offset - st.x_lag * z) * mix * d.x;
            }
            if y {
                bone.world_y += (st.y_offset - st.y_lag * z) * mix * d.y;
            }
        }
        Physics::None => unreachable!("handled above"),
    }

    if rotate_or_shear_x {
        let mut o = (st.rotate_offset - st.rotate_lag * z) * mix;
        let (mut s, mut c, mut a);
        if d.shear_x > 0.0 {
            let mut r = 0.0f32;
            if d.rotate > 0.0 {
                r = o * d.rotate;
                s = math::sin(r);
                c = math::cos(r);
                a = bone.b;
                bone.b = c * a - s * bone.d;
                bone.d = s * a + c * bone.d;
            }
            r += o * d.shear_x;
            s = math::sin(r);
            c = math::cos(r);
            a = bone.a;
            bone.a = c * a - s * bone.c;
            bone.c = s * a + c * bone.c;
        } else {
            o *= d.rotate;
            s = math::sin(o);
            c = math::cos(o);
            a = bone.a;
            bone.a = c * a - s * bone.c;
            bone.c = s * a + c * bone.c;
            a = bone.b;
            bone.b = c * a - s * bone.d;
            bone.d = s * a + c * bone.d;
        }
    }
    if scale_x {
        let mut s = 1.0 + (st.scale_offset - st.scale_lag * z) * mix * d.scale_x;
        bone.a *= s;
        bone.c *= s;
        match d.scale_y_mode {
            ScaleYMode::Uniform => {
                bone.b *= s;
                bone.d *= s;
            }
            ScaleYMode::Volume => {
                s = abs(s);
                s = if s >= 0.7 { 1.0 / s } else { 4.0 - 3.67347 * s };
                bone.b *= s;
                bone.d *= s;
            }
            ScaleYMode::None => {}
        }
    }
    if physics != Physics::Pose {
        st.tx = l * bone.a;
        st.ty = l * bone.c;
    }
    *sk.bones[bone_id.0].applied_pose_mut() = bone;
    if let ConstraintBody::Physics(pc) = &mut sk.constraints[index].body {
        pc.state = st;
    }
}
