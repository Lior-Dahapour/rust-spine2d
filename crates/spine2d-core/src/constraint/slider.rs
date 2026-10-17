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
use crate::animation::{ApplyOptions, MixFrom, Timeline};
use crate::bone::BoneId;
use crate::constraint_data::{ConstraintData, SliderData, SliderPose};
use crate::math::fmod;
use crate::skeleton::{Skeleton, Update};
use crate::skeleton_data::SkeletonData;

/// Applies an animation to the skeleton, driven by a bone property or a time value.
#[derive(Clone, Debug)]
pub struct Slider {
    pub pose: PoseSet<SliderPose>,
    pub(crate) bone: Option<BoneId>,
}

impl Slider {
    pub(crate) fn new(d: &SliderData) -> Self {
        Slider {
            pose: PoseSet::new(d.setup_pose),
            bone: d.bone.map(BoneId),
        }
    }

    /// The bone whose property drives the animation, if any.
    pub fn bone(&self) -> Option<BoneId> {
        self.bone
    }

    /// Drives the animation from a property of `bone`, or from the pose time when `None`. Call
    /// [`update_cache`](crate::skeleton::Borrowed::update_cache) afterwards so `bone` is updated before the constraint.
    pub fn set_bone(&mut self, bone: Option<BoneId>) {
        self.bone = bone;
    }
}

fn parts(c: &Constraint) -> &Slider {
    match &c.body {
        ConstraintBody::Slider(s) => s,
        _ => unreachable!("not a slider"),
    }
}

pub(super) fn update(sk: &mut Skeleton, data: &SkeletonData, index: usize) {
    let ConstraintData::Slider(d) = &data.constraints[index] else {
        return;
    };
    let slider = parts(&sk.constraints[index]);
    let mut p = *slider.pose.applied();
    if p.mix == 0.0 {
        return;
    }
    let bone = slider.bone;
    let Some(animation) = d.animation.clone() else {
        return;
    };
    if let Some(bone_id) = bone {
        if !sk.bones[bone_id.0].active {
            return;
        }
        if d.local {
            sk.validate_bone_local_transform(bone_id);
        }
        let source = *sk.bones[bone_id.0].applied_pose();
        let st = sk.transform();
        if let Some(property) = &d.property {
            let value =
                super::transform::from_value(property.property, &st, &source, d.local, &[0.0; 6]);
            p.time = d.offset + (value - property.offset) * d.scale;
        }
        if d.loop_ {
            p.time = animation.duration + fmod(p.time, animation.duration);
        } else {
            p.time = (0.0f32).max(p.time);
        }
        if let ConstraintBody::Slider(s) = &mut sk.constraints[index].body {
            s.pose.applied_mut().time = p.time;
        }
    }
    for &b in animation.bones() {
        sk.modify_bone_local(BoneId(b));
    }
    let options = ApplyOptions {
        alpha: p.mix,
        from: MixFrom::Current,
        add: d.additive,
        out: false,
        applied_pose: true,
    };
    animation.apply(
        &mut sk.as_borrow(data),
        p.time,
        p.time,
        d.loop_,
        None,
        &options,
    );
}

pub(super) fn sort(sk: &mut Skeleton, data: &SkeletonData, index: usize) {
    let ConstraintData::Slider(d) = &data.constraints[index] else {
        return;
    };
    let bone = parts(&sk.constraints[index]).bone;
    if let Some(b) = bone {
        if !d.local {
            sk.sort_bone(b);
        }
    }
    sk.update_cache.push(Update::Constraint(index));
    let Some(animation) = d.animation.clone() else {
        return;
    };
    for &b in animation.bones() {
        sk.bones[b].sorted = false;
        sk.sort_reset_children(BoneId(b));
        sk.constrained_bone(BoneId(b));
    }
    for timeline in animation.timelines() {
        match timeline {
            Timeline::Color(_)
            | Timeline::Alpha(_)
            | Timeline::Attachment(_)
            | Timeline::Deform(_)
            | Timeline::Sequence(_) => {
                let slot = timeline.slot_index().expect("slot timeline");
                sk.constrained_slot(crate::slot::SlotId(slot));
            }
            Timeline::DrawOrder(_) | Timeline::DrawOrderFolder(_) => {
                sk.draw_order.set_constrained()
            }
            other => {
                let Some(target) = other.constraint_index() else {
                    if other.is_global_physics() {
                        for i in 0..sk.constraints.len() {
                            if matches!(sk.constraints[i].body, ConstraintBody::Physics(_)) {
                                sk.constrained_constraint(i);
                            }
                        }
                    }
                    continue;
                };
                sk.constrained_constraint(target);
            }
        }
    }
}
