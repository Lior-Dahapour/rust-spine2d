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

use super::{Constraint, ConstraintBody};
use crate::bone::BoneId;
use crate::constraint_data::{ConstraintData, ScaleYMode};
use crate::enums::Inherit;
use crate::math::{self, abs, sign, EPSILON};
use crate::skeleton::{Skeleton, Update};
use crate::skeleton_data::SkeletonData;

fn parts(c: &Constraint) -> (&super::IkConstraint, usize) {
    match &c.body {
        ConstraintBody::Ik(ik) => (ik, c.index),
        _ => unreachable!("not an IK constraint"),
    }
}

pub(super) fn update(sk: &mut Skeleton, data: &SkeletonData, index: usize) {
    let (ik, _) = parts(&sk.constraints[index]);
    let p = *ik.pose.applied();
    if p.mix == 0.0 {
        return;
    }
    let target = ik.target;
    let ConstraintData::Ik(d) = &data.constraints[index] else {
        return;
    };
    let bones = &d.bones;
    let t = *sk.bones[target.0].applied_pose();
    match bones.len() {
        1 => apply_one(
            sk,
            data,
            BoneId(bones[0]),
            t.world_x,
            t.world_y,
            p.compress,
            p.stretch,
            d.scale_y_mode,
            p.mix,
        ),
        2 => apply_two(
            sk,
            data,
            BoneId(bones[0]),
            BoneId(bones[1]),
            t.world_x,
            t.world_y,
            p.bend_direction,
            p.stretch,
            d.scale_y_mode,
            p.softness,
            p.mix,
        ),
        _ => {}
    }
}

pub(super) fn sort(sk: &mut Skeleton, data: &SkeletonData, index: usize) {
    let (ik, _) = parts(&sk.constraints[index]);
    let target = ik.target;
    let ConstraintData::Ik(d) = &data.constraints[index] else {
        return;
    };
    let bones = &d.bones;
    sk.sort_bone(target);
    let parent = BoneId(bones[0]);
    sk.sort_bone(parent);
    sk.update_cache.push(Update::Constraint(index));
    sk.bones[parent.0].sorted = false;
    sk.sort_reset_children(parent);
    sk.constrained_bone(parent);
    if bones.len() > 1 {
        sk.constrained_bone(BoneId(bones[1]));
    }
}

/// Rotates (and optionally scales) one bone toward a world target.
#[allow(clippy::too_many_arguments)]
pub fn apply_one(
    sk: &mut Skeleton,
    data: &SkeletonData,
    bone: BoneId,
    target_x: f32,
    target_y: f32,
    compress: bool,
    stretch: bool,
    scale_y_mode: ScaleYMode,
    mix: f32,
) {
    sk.modify_bone_local(bone);
    let parent = sk.bones[bone.0].parent.expect("IK bone has a parent");
    let p = *sk.bones[parent.0].applied_pose();
    let (pa, mut pb, pc, mut pd) = (p.a, p.b, p.c, p.d);
    let (sx, sy) = (sk.scale_x, sk.scale_y());
    let bone_length = data.bones[bone.0].length;
    let b = sk.bones[bone.0].applied_pose_mut();
    let l = &mut b.local;
    let mut rotation_ik = -l.shear_x - l.rotation;
    let (mut tx, mut ty);
    match l.inherit {
        Inherit::OnlyTranslation => {
            tx = (target_x - b.world_x) * sign(sx);
            ty = (target_y - b.world_y) * sign(sy);
        }
        mode => {
            if mode == Inherit::NoRotationOrReflection {
                let s = abs(pa * pd - pb * pc) / (EPSILON).max(pa * pa + pc * pc);
                let sa = pa / sx;
                let sc = pc / sy;
                pb = -sc * s * sx;
                pd = sa * s * sy;
                rotation_ik += math::atan2_deg(sc, sa);
            }
            let x = target_x - p.world_x;
            let y = target_y - p.world_y;
            let d = pa * pd - pb * pc;
            if abs(d) <= EPSILON {
                tx = 0.0;
                ty = 0.0;
            } else {
                tx = (x * pd - y * pb) / d - l.x;
                ty = (y * pa - x * pc) / d - l.y;
            }
        }
    }
    rotation_ik += math::atan2_deg(ty, tx);
    if l.scale_x < 0.0 {
        rotation_ik += 180.0;
    }
    if rotation_ik > 180.0 {
        rotation_ik -= 360.0;
    } else if rotation_ik <= -180.0 {
        rotation_ik += 360.0;
    }
    l.rotation += rotation_ik * mix;
    if compress || stretch {
        if matches!(l.inherit, Inherit::NoScale | Inherit::NoScaleOrReflection) {
            tx = target_x - b.world_x;
            ty = target_y - b.world_y;
        }
        let bl = bone_length * l.scale_x;
        if bl > EPSILON {
            let dd = tx * tx + ty * ty;
            if (compress && dd < bl * bl) || (stretch && dd > bl * bl) {
                let s = (math::sqrt(dd) / bl - 1.0) * mix + 1.0;
                l.scale_x *= s;
                match scale_y_mode {
                    ScaleYMode::Uniform => l.scale_y *= s,
                    ScaleYMode::Volume => {
                        l.scale_y /= if s < 0.7 { 0.25 + 0.642857 * s } else { s }
                    }
                    ScaleYMode::None => {}
                }
            }
        }
    }
}

/// Rotates a parent and child bone so the child's tip reaches a world target (two-bone IK).
#[allow(clippy::too_many_arguments)]
pub fn apply_two(
    sk: &mut Skeleton,
    data: &SkeletonData,
    parent_id: BoneId,
    child_id: BoneId,
    target_x: f32,
    target_y: f32,
    bend_dir: i32,
    stretch: bool,
    scale_y_mode: ScaleYMode,
    mut softness: f32,
    mix: f32,
) {
    {
        let parent = sk.bones[parent_id.0].applied_pose();
        let child = sk.bones[child_id.0].applied_pose();
        if parent.local.inherit != Inherit::Normal || child.local.inherit != Inherit::Normal {
            return;
        }
    }
    sk.modify_bone_local(parent_id);
    sk.modify_bone_local(child_id);
    let mut parent = *sk.bones[parent_id.0].applied_pose();
    let mut child = *sk.bones[child_id.0].applied_pose();
    let (px, py) = (parent.local.x, parent.local.y);
    let (mut psx, mut psy, mut csx) = (
        parent.local.scale_x,
        parent.local.scale_y,
        child.local.scale_x,
    );
    let (os1, mut s2, os2): (i32, i32, i32);
    if psx < 0.0 {
        psx = -psx;
        os1 = 180;
        s2 = -1;
    } else {
        os1 = 0;
        s2 = 1;
    }
    if psy < 0.0 {
        psy = -psy;
        s2 = -s2;
    }
    if csx < 0.0 {
        csx = -csx;
        os2 = 180;
    } else {
        os2 = 0;
    }
    let (cwx, cwy);
    let (mut a, mut b, mut c, mut d) = (parent.a, parent.b, parent.c, parent.d);
    let u = abs(psx - psy) <= EPSILON;
    if !u || stretch {
        child.local.y = 0.0;
        cwx = a * child.local.x + parent.world_x;
        cwy = c * child.local.x + parent.world_y;
    } else {
        cwx = a * child.local.x + b * child.local.y + parent.world_x;
        cwy = c * child.local.x + d * child.local.y + parent.world_y;
    }
    let grand = sk.bones[parent_id.0]
        .parent
        .expect("IK parent bone has a parent");
    let pp = *sk.bones[grand.0].applied_pose();
    a = pp.a;
    b = pp.b;
    c = pp.c;
    d = pp.d;
    let mut id = a * d - b * c;
    let mut x = cwx - pp.world_x;
    let mut y = cwy - pp.world_y;
    id = if abs(id) <= EPSILON { 0.0 } else { 1.0 / id };
    let dx = (x * d - y * b) * id - px;
    let dy = (y * a - x * c) * id - py;
    let l1 = math::sqrt(dx * dx + dy * dy);
    let mut l2 = data.bones[child_id.0].length * csx;
    let (mut a1, mut a2);
    if l1 < EPSILON {
        *sk.bones[child_id.0].applied_pose_mut() = child;
        apply_one(
            sk,
            data,
            parent_id,
            target_x,
            target_y,
            false,
            stretch,
            ScaleYMode::None,
            mix,
        );
        sk.bones[child_id.0].applied_pose_mut().local.rotation = 0.0;
        return;
    }
    x = target_x - pp.world_x;
    y = target_y - pp.world_y;
    let mut tx = (x * d - y * b) * id - px;
    let mut ty = (y * a - x * c) * id - py;
    let mut dd = tx * tx + ty * ty;
    if softness != 0.0 {
        softness *= psx * (csx + 1.0) * 0.5;
        let td = math::sqrt(dd);
        let sd = td - l1 - l2 * psx + softness;
        if sd > 0.0 {
            let mut p = (1.0f32).min(sd / (softness * 2.0)) - 1.0;
            p = (sd - softness * (1.0 - p * p)) / td;
            tx -= p * tx;
            ty -= p * ty;
            dd = tx * tx + ty * ty;
        }
    }
    'outer: {
        if u {
            l2 *= psx;
            let mut cos = (dd - l1 * l1 - l2 * l2) / (2.0 * l1 * l2);
            if cos < -1.0 {
                cos = -1.0;
                a2 = math::PI * bend_dir as f32;
            } else if cos > 1.0 {
                cos = 1.0;
                a2 = 0.0;
                if stretch {
                    a = (math::sqrt(dd) / (l1 + l2) - 1.0) * mix + 1.0;
                    parent.local.scale_x *= a;
                    match scale_y_mode {
                        ScaleYMode::Uniform => parent.local.scale_y *= a,
                        ScaleYMode::Volume => {
                            parent.local.scale_y /= if a < 0.7 { 0.25 + 0.642857 * a } else { a }
                        }
                        ScaleYMode::None => {}
                    }
                }
            } else {
                a2 = math::acos(cos) * bend_dir as f32;
            }
            a = l1 + l2 * cos;
            b = l2 * math::sin(a2);
            a1 = math::atan2(ty * a - tx * b, tx * a + ty * b);
        } else {
            a = psx * l2;
            b = psy * l2;
            let aa = a * a;
            let bb = b * b;
            let ta = math::atan2(ty, tx);
            c = bb * l1 * l1 + aa * dd - aa * bb;
            let c1 = -2.0 * bb * l1;
            let c2 = bb - aa;
            d = c1 * c1 - 4.0 * c2 * c;
            if d >= 0.0 {
                let mut q = math::sqrt(d);
                if c1 < 0.0 {
                    q = -q;
                }
                q = -(c1 + q) * 0.5;
                let r0 = q / c2;
                let r1 = c / q;
                let r = if abs(r0) < abs(r1) { r0 } else { r1 };
                let r0 = dd - r * r;
                if r0 >= 0.0 {
                    y = math::sqrt(r0) * bend_dir as f32;
                    a1 = ta - math::atan2(y, r);
                    a2 = math::atan2(y / psy, (r - l1) / psx);
                    break 'outer;
                }
            }
            let mut min_angle = math::PI;
            let mut min_x = l1 - a;
            let mut min_dist = min_x * min_x;
            let mut min_y = 0.0;
            let mut max_angle = 0.0;
            let mut max_x = l1 + a;
            let mut max_dist = max_x * max_x;
            let mut max_y = 0.0;
            c = -a * l1 / (aa - bb);
            if (-1.0..=1.0).contains(&c) {
                c = math::acos(c);
                x = a * math::cos(c) + l1;
                y = b * math::sin(c);
                d = x * x + y * y;
                if d < min_dist {
                    min_angle = c;
                    min_dist = d;
                    min_x = x;
                    min_y = y;
                }
                if d > max_dist {
                    max_angle = c;
                    max_dist = d;
                    max_x = x;
                    max_y = y;
                }
            }
            if dd <= (min_dist + max_dist) * 0.5 {
                a1 = ta - math::atan2(min_y * bend_dir as f32, min_x);
                a2 = min_angle * bend_dir as f32;
            } else {
                a1 = ta - math::atan2(max_y * bend_dir as f32, max_x);
                a2 = max_angle * bend_dir as f32;
            }
        }
    }
    let os = math::atan2(child.local.y, child.local.x) * s2 as f32;
    a1 = (a1 - os) * math::RAD_DEG + os1 as f32 - parent.local.rotation;
    if a1 > 180.0 {
        a1 -= 360.0;
    } else if a1 <= -180.0 {
        a1 += 360.0;
    }
    parent.local.rotation += a1 * mix;
    a2 = ((a2 + os) * math::RAD_DEG - child.local.shear_x) * s2 as f32 + os2 as f32
        - child.local.rotation;
    if a2 > 180.0 {
        a2 -= 360.0;
    } else if a2 <= -180.0 {
        a2 += 360.0;
    }
    child.local.rotation += a2 * mix;
    *sk.bones[parent_id.0].applied_pose_mut() = parent;
    *sk.bones[child_id.0].applied_pose_mut() = child;
}
