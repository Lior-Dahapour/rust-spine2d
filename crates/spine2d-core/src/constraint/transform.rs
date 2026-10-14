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
use crate::bone::{BoneId, BonePose, SkeletonTransform};
use crate::constraint_data::{
    BoneProperty, ConstraintData, TransformConstraintData, TransformConstraintPose,
};
use crate::math::{self, DEG_RAD, PI, PI_2, RAD_DEG};
use crate::skeleton::{Skeleton, Update};
use crate::skeleton_data::SkeletonData;

fn parts(c: &Constraint) -> &super::TransformConstraint {
    match &c.body {
        ConstraintBody::Transform(t) => t,
        _ => unreachable!("not a transform constraint"),
    }
}

pub(super) fn update(sk: &mut Skeleton, data: &SkeletonData, index: usize) {
    let t = parts(&sk.constraints[index]);
    let p = *t.pose.applied();
    if p.mix_rotate == 0.0
        && p.mix_x == 0.0
        && p.mix_y == 0.0
        && p.mix_scale_x == 0.0
        && p.mix_scale_y == 0.0
        && p.mix_shear_y == 0.0
    {
        return;
    }
    let source_id = t.source;
    let ConstraintData::Transform(d) = &data.constraints[index] else {
        return;
    };
    if d.local_source {
        sk.validate_bone_local_transform(source_id);
    }
    let source = *sk.bones[source_id.0].applied_pose();
    let st = sk.transform();
    for &bone_index in &d.bones {
        let bone_id = BoneId(bone_index);
        if d.local_target {
            sk.modify_bone_local(bone_id);
        } else {
            sk.modify_bone_world(bone_id);
        }
        let mut bone = *sk.bones[bone_id.0].applied_pose();
        for from in &d.properties {
            let value =
                from_value(from.property, &st, &source, d.local_source, &d.offsets) - from.offset;
            for to in &from.to {
                if to_mix(to.property, &p) != 0.0 {
                    let mut clamped = to.offset + value * to.scale;
                    if d.clamp {
                        clamped = if to.offset < to.max {
                            math::clamp(clamped, to.offset, to.max)
                        } else {
                            math::clamp(clamped, to.max, to.offset)
                        };
                    }
                    to_apply(
                        to.property,
                        &st,
                        &p,
                        &mut bone,
                        clamped,
                        d.local_target,
                        d.additive,
                    );
                }
            }
        }
        *sk.bones[bone_id.0].applied_pose_mut() = bone;
    }
}

pub(super) fn sort(sk: &mut Skeleton, data: &SkeletonData, index: usize) {
    let t = parts(&sk.constraints[index]);
    let source = t.source;
    let ConstraintData::Transform(d) = &data.constraints[index] else {
        return;
    };
    if !d.local_source {
        sk.sort_bone(source);
    }
    let world_target = !d.local_target;
    if world_target {
        for &b in &d.bones {
            sk.sort_bone(BoneId(b));
        }
    }
    sk.update_cache.push(Update::Constraint(index));
    for &b in &d.bones {
        sk.sort_reset_children(BoneId(b));
        sk.constrained_bone(BoneId(b));
    }
    for &b in &d.bones {
        sk.bones[b].sorted = world_target;
    }
}

/// The value of `property` of a source bone pose, as read by transform constraints and sliders.
/// `offsets` are the constraint's rotation, x, y, scale x, scale y and shear y offsets.
pub fn from_value(
    property: BoneProperty,
    st: &SkeletonTransform,
    source: &BonePose,
    local: bool,
    offsets: &[f32; 6],
) -> f32 {
    let (sx, sy) = (st.scale_x, st.scale_y);
    let l = &source.local;
    match property {
        BoneProperty::Rotate => {
            if local {
                return l.rotation + offsets[TransformConstraintData::ROTATION];
            }
            let o = offsets[TransformConstraintData::ROTATION];
            let mut value = math::atan2(source.c / sy, source.a / sx) * RAD_DEG
                + if (source.a * source.d - source.b * source.c) * sx * sy > 0.0 {
                    o
                } else {
                    -o
                };
            if value < 0.0 {
                value += 360.0;
            }
            value
        }
        BoneProperty::X => {
            if local {
                l.x + offsets[TransformConstraintData::X]
            } else {
                (offsets[TransformConstraintData::X] * source.a
                    + offsets[TransformConstraintData::Y] * source.b
                    + source.world_x)
                    / sx
            }
        }
        BoneProperty::Y => {
            if local {
                l.y + offsets[TransformConstraintData::Y]
            } else {
                (offsets[TransformConstraintData::X] * source.c
                    + offsets[TransformConstraintData::Y] * source.d
                    + source.world_y)
                    / sy
            }
        }
        BoneProperty::ScaleX => {
            if local {
                return l.scale_x + offsets[TransformConstraintData::SCALE_X];
            }
            let a = source.a / sx;
            let c = source.c / sy;
            math::sqrt(a * a + c * c) + offsets[TransformConstraintData::SCALE_X]
        }
        BoneProperty::ScaleY => {
            if local {
                return l.scale_y + offsets[TransformConstraintData::SCALE_Y];
            }
            let b = source.b / sx;
            let d = source.d / sy;
            math::sqrt(b * b + d * d) + offsets[TransformConstraintData::SCALE_Y]
        }
        BoneProperty::ShearY => {
            if local {
                return l.shear_y + offsets[TransformConstraintData::SHEAR_Y];
            }
            let ix = 1.0 / sx;
            let iy = 1.0 / sy;
            (math::atan2(source.d * iy, source.b * ix) - math::atan2(source.c * iy, source.a * ix))
                * RAD_DEG
                - 90.0
                + offsets[TransformConstraintData::SHEAR_Y]
        }
    }
}

/// The mix of the constraint pose that scales writes to `property`.
pub fn to_mix(property: BoneProperty, pose: &TransformConstraintPose) -> f32 {
    match property {
        BoneProperty::Rotate => pose.mix_rotate,
        BoneProperty::X => pose.mix_x,
        BoneProperty::Y => pose.mix_y,
        BoneProperty::ScaleX => pose.mix_scale_x,
        BoneProperty::ScaleY => pose.mix_scale_y,
        BoneProperty::ShearY => pose.mix_shear_y,
    }
}

/// Writes `value` to `property` of `bone`, local or world, blended by the pose's mix.
pub fn to_apply(
    property: BoneProperty,
    st: &SkeletonTransform,
    pose: &TransformConstraintPose,
    bone: &mut BonePose,
    mut value: f32,
    local: bool,
    additive: bool,
) {
    let (sx, sy) = (st.scale_x, st.scale_y);
    match property {
        BoneProperty::Rotate => {
            if local {
                bone.local.rotation += (if additive {
                    value
                } else {
                    value - bone.local.rotation
                }) * pose.mix_rotate;
            } else {
                let (ix, iy) = (1.0 / sx, 1.0 / sy);
                let (a, b, c, d) = (bone.a * ix, bone.b * ix, bone.c * iy, bone.d * iy);
                value *= DEG_RAD;
                if !additive {
                    value -= math::atan2(c, a);
                }
                if value > PI {
                    value -= PI_2;
                } else if value < -PI {
                    value += PI_2;
                }
                value *= pose.mix_rotate;
                let cos_val = math::cos(value);
                let sin_val = math::sin(value);
                bone.a = (cos_val * a - sin_val * c) * sx;
                bone.b = (cos_val * b - sin_val * d) * sx;
                bone.c = (sin_val * a + cos_val * c) * sy;
                bone.d = (sin_val * b + cos_val * d) * sy;
            }
        }
        BoneProperty::X => {
            if local {
                bone.local.x += (if additive {
                    value
                } else {
                    value - bone.local.x
                }) * pose.mix_x;
            } else {
                if !additive {
                    value -= bone.world_x / sx;
                }
                bone.world_x += value * pose.mix_x * sx;
            }
        }
        BoneProperty::Y => {
            if local {
                bone.local.y += (if additive {
                    value
                } else {
                    value - bone.local.y
                }) * pose.mix_y;
            } else {
                if !additive {
                    value -= bone.world_y / sy;
                }
                bone.world_y += value * pose.mix_y * sy;
            }
        }
        BoneProperty::ScaleX => {
            if local {
                if additive {
                    bone.local.scale_x *= 1.0 + (value - 1.0) * pose.mix_scale_x;
                } else if bone.local.scale_x != 0.0 {
                    bone.local.scale_x += (value - bone.local.scale_x) * pose.mix_scale_x;
                }
            } else if additive {
                let s = 1.0 + (value - 1.0) * pose.mix_scale_x;
                bone.a *= s;
                bone.c *= s;
            } else {
                let a = bone.a / sx;
                let c = bone.c / sy;
                let mut s = math::sqrt(a * a + c * c);
                if s != 0.0 {
                    s = 1.0 + (value - s) * pose.mix_scale_x / s;
                    bone.a *= s;
                    bone.c *= s;
                }
            }
        }
        BoneProperty::ScaleY => {
            if local {
                if additive {
                    bone.local.scale_y *= 1.0 + (value - 1.0) * pose.mix_scale_y;
                } else if bone.local.scale_y != 0.0 {
                    bone.local.scale_y += (value - bone.local.scale_y) * pose.mix_scale_y;
                }
            } else if additive {
                let s = 1.0 + (value - 1.0) * pose.mix_scale_y;
                bone.b *= s;
                bone.d *= s;
            } else {
                let b = bone.b / sx;
                let d = bone.d / sy;
                let mut s = math::sqrt(b * b + d * d);
                if s != 0.0 {
                    s = 1.0 + (value - s) * pose.mix_scale_y / s;
                    bone.b *= s;
                    bone.d *= s;
                }
            }
        }
        BoneProperty::ShearY => {
            if local {
                if !additive {
                    value -= bone.local.shear_y;
                }
                bone.local.shear_y += value * pose.mix_shear_y;
            } else {
                let b = bone.b / sx;
                let d = bone.d / sy;
                let by = math::atan2(d, b);
                value = (value + 90.0) * DEG_RAD;
                if additive {
                    value -= PI / 2.0;
                } else {
                    value -= by - math::atan2(bone.c / sy, bone.a / sx);
                    if value > PI {
                        value -= PI_2;
                    } else if value < -PI {
                        value += PI_2;
                    }
                }
                value = by + value * pose.mix_shear_y;
                let s = math::sqrt(b * b + d * d);
                bone.b = math::cos(value) * s * sx;
                bone.d = math::sin(value) * s * sy;
            }
        }
    }
}
