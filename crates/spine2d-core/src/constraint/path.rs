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
use crate::attachment::{Attachment, PathAttachment};
use crate::bone::BoneId;
use crate::constraint_data::{
    ConstraintData, PathConstraintData, PathConstraintPose, PositionMode, RotateMode, SpacingMode,
};
use crate::math::{self, EPSILON};
use crate::skeleton::{Skeleton, Update};
use crate::skeleton_data::SkeletonData;
use crate::skin::Skin;
use crate::slot::SlotId;

const NONE: i32 = -1;
const BEFORE: i32 = -2;
const AFTER: i32 = -3;

/// Scratch buffers reused between updates.
#[derive(Clone, Debug, Default)]
struct Scratch {
    spaces: Vec<f32>,
    positions: Vec<f32>,
    world: Vec<f32>,
    curves: Vec<f32>,
    lengths: Vec<f32>,
    segments: Vec<f32>,
}

/// Positions bones along a path attachment.
#[derive(Clone, Debug)]
pub struct PathConstraint {
    pub pose: PoseSet<PathConstraintPose>,
    pub(crate) slot: SlotId,
    scratch: Scratch,
}

impl PathConstraint {
    pub(crate) fn new(d: &PathConstraintData) -> Self {
        PathConstraint {
            pose: PoseSet::new(d.setup_pose),
            slot: SlotId(d.slot),
            scratch: Scratch {
                segments: vec![0.0; 10],
                ..Default::default()
            },
        }
    }

    /// The slot holding the path attachment.
    pub fn slot(&self) -> SlotId {
        self.slot
    }
}

fn parts(c: &Constraint) -> &PathConstraint {
    match &c.body {
        ConstraintBody::Path(p) => p,
        _ => unreachable!("not a path constraint"),
    }
}

pub(super) fn update(sk: &mut Skeleton, data: &SkeletonData, index: usize) {
    let ConstraintData::Path(d) = &data.constraints[index] else {
        return;
    };
    let pc = parts(&sk.constraints[index]);
    let Some(base) = sk.slots[pc.slot.0].applied_pose().attachment.clone() else {
        return;
    };
    let Attachment::Path(path) = &*base else {
        return;
    };
    let p = *pc.pose.applied();
    if p.mix_rotate == 0.0 && p.mix_x == 0.0 && p.mix_y == 0.0 {
        return;
    }
    let slot = pc.slot;
    let mut scratch = std::mem::take(match &mut sk.constraints[index].body {
        ConstraintBody::Path(p) => &mut p.scratch,
        _ => unreachable!(),
    });
    run(sk, data, d, path, &p, &d.bones, slot, &mut scratch);
    if let ConstraintBody::Path(pc) = &mut sk.constraints[index].body {
        pc.scratch = scratch;
    }
}

#[allow(clippy::too_many_arguments)]
fn run(
    sk: &mut Skeleton,
    data: &SkeletonData,
    d: &PathConstraintData,
    path: &PathAttachment,
    p: &PathConstraintPose,
    bones: &[usize],
    slot: SlotId,
    sc: &mut Scratch,
) {
    let (mix_rotate, mix_x, mix_y) = (p.mix_rotate, p.mix_x, p.mix_y);
    let tangents = d.rotate_mode == RotateMode::Tangent;
    let scale = d.rotate_mode == RotateMode::ChainScale;
    let bone_count = bones.len();
    let spaces_count = if tangents { bone_count } else { bone_count + 1 };
    sc.spaces.clear();
    sc.spaces.resize(spaces_count, 0.0);
    if scale {
        sc.lengths.clear();
        sc.lengths.resize(bone_count, 0.0);
    }
    let spacing = p.spacing;
    let pose_of = |sk: &Skeleton, b: BoneId| *sk.bones[b.0].applied_pose();
    match d.spacing_mode {
        SpacingMode::Percent => {
            if scale {
                for (i, &bone_index) in bones.iter().enumerate().take(spaces_count - 1) {
                    let bone = pose_of(sk, BoneId(bone_index));
                    let setup_length = data.bones[bone_index].length;
                    let x = setup_length * bone.a;
                    let y = setup_length * bone.c;
                    sc.lengths[i] = math::sqrt(x * x + y * y);
                }
            }
            for i in 1..spaces_count {
                sc.spaces[i] = spacing;
            }
        }
        SpacingMode::Proportional => {
            let mut sum = 0.0;
            let mut i = 0;
            let n = spaces_count - 1;
            while i < n {
                let bone = pose_of(sk, BoneId(bones[i]));
                let setup_length = data.bones[bones[i]].length;
                if setup_length < EPSILON {
                    if scale {
                        sc.lengths[i] = 0.0;
                    }
                    i += 1;
                    sc.spaces[i] = spacing;
                } else {
                    let x = setup_length * bone.a;
                    let y = setup_length * bone.c;
                    let length = math::sqrt(x * x + y * y);
                    if scale {
                        sc.lengths[i] = length;
                    }
                    i += 1;
                    sc.spaces[i] = length;
                    sum += length;
                }
            }
            if sum > 0.0 {
                sum = spaces_count as f32 / sum * spacing;
                for i in 1..spaces_count {
                    sc.spaces[i] *= sum;
                }
            }
        }
        _ => {
            let length_spacing = d.spacing_mode == SpacingMode::Length;
            let mut i = 0;
            let n = spaces_count - 1;
            while i < n {
                let bone = pose_of(sk, BoneId(bones[i]));
                let setup_length = data.bones[bones[i]].length;
                if setup_length < EPSILON {
                    if scale {
                        sc.lengths[i] = 0.0;
                    }
                    i += 1;
                    sc.spaces[i] = spacing;
                } else {
                    let x = setup_length * bone.a;
                    let y = setup_length * bone.c;
                    let length = math::sqrt(x * x + y * y);
                    if scale {
                        sc.lengths[i] = length;
                    }
                    i += 1;
                    sc.spaces[i] = (if length_spacing {
                        (0.0f32).max(setup_length + spacing)
                    } else {
                        spacing
                    }) * length
                        / setup_length;
                }
            }
        }
    }

    compute_world_positions(sk, d, p, path, slot, sc, spaces_count, tangents);

    let mut bone_x = sc.positions[0];
    let mut bone_y = sc.positions[1];
    let mut offset_rotation = d.offset_rotation;
    let tip;
    if offset_rotation == 0.0 {
        tip = d.rotate_mode == RotateMode::Chain;
    } else {
        tip = false;
        let bone = *sk.bones[sk.slots[slot.0].bone.0].applied_pose();
        offset_rotation *= if bone.a * bone.d - bone.b * bone.c > 0.0 {
            math::DEG_RAD
        } else {
            -math::DEG_RAD
        };
    }
    let mut ip = 3;
    for (i, &bone_index) in bones.iter().enumerate().take(bone_count) {
        let id = BoneId(bone_index);
        sk.modify_bone_world(id);
        let mut bone = *sk.bones[id.0].applied_pose();
        bone.world_x += (bone_x - bone.world_x) * mix_x;
        bone.world_y += (bone_y - bone.world_y) * mix_y;
        let x = sc.positions[ip];
        let y = sc.positions[ip + 1];
        let dx = x - bone_x;
        let dy = y - bone_y;
        if scale {
            let length = sc.lengths[i];
            if length >= EPSILON {
                let s = (math::sqrt(dx * dx + dy * dy) / length - 1.0) * mix_rotate + 1.0;
                bone.a *= s;
                bone.c *= s;
            }
        }
        bone_x = x;
        bone_y = y;
        if mix_rotate > 0.0 {
            let (a, b, c, d2) = (bone.a, bone.b, bone.c, bone.d);
            let mut r;
            if tangents {
                r = sc.positions[ip - 1];
            } else if sc.spaces[i + 1] < EPSILON {
                r = sc.positions[ip + 2];
            } else {
                r = math::atan2(dy, dx);
            }
            r -= math::atan2(c, a);
            if tip {
                let cos = math::cos(r);
                let sin = math::sin(r);
                let length = data.bones[id.0].length;
                bone_x += (length * (cos * a - sin * c) - dx) * mix_rotate;
                bone_y += (length * (sin * a + cos * c) - dy) * mix_rotate;
            } else {
                r += offset_rotation;
            }
            if r > math::PI {
                r -= math::PI_2;
            } else if r < -math::PI {
                r += math::PI_2;
            }
            r *= mix_rotate;
            let cos = math::cos(r);
            let sin = math::sin(r);
            bone.a = cos * a - sin * c;
            bone.b = cos * b - sin * d2;
            bone.c = sin * a + cos * c;
            bone.d = sin * b + cos * d2;
        }
        *sk.bones[id.0].applied_pose_mut() = bone;
        ip += 3;
    }
}

#[allow(clippy::too_many_arguments)]
fn compute_world_positions(
    sk: &Skeleton,
    d: &PathConstraintData,
    p: &PathConstraintPose,
    path: &PathAttachment,
    slot: SlotId,
    sc: &mut Scratch,
    spaces_count: usize,
    tangents: bool,
) {
    let mut position = p.position;
    sc.positions.clear();
    sc.positions.resize(spaces_count * 3 + 2, 0.0);
    let closed = path.closed;
    let mut vertices_length = path.vertex.world_vertices_length as i32;
    let mut curve_count = vertices_length / 6;
    let mut prev_curve = NONE;
    let path_length;
    let vertex = &path.vertex;

    if !path.constant_speed {
        let lengths = &path.lengths;
        curve_count -= if closed { 1 } else { 2 };
        path_length = lengths[curve_count as usize];
        if d.position_mode == PositionMode::Percent {
            position *= path_length;
        }
        let multiplier = match d.spacing_mode {
            SpacingMode::Percent => path_length,
            SpacingMode::Proportional => path_length / spaces_count as f32,
            _ => 1.0,
        };
        sc.world.clear();
        sc.world.resize(8, 0.0);
        let mut o = 0;
        let mut curve: i32 = 0;
        for i in 0..spaces_count {
            let space = sc.spaces[i] * multiplier;
            position += space;
            let mut pp = position;
            if closed {
                pp = math::fmod(pp, path_length);
                if pp < 0.0 {
                    pp += path_length;
                }
                curve = 0;
            } else if pp < 0.0 {
                if prev_curve != BEFORE {
                    prev_curve = BEFORE;
                    vertex.compute_world_vertices(sk, slot, 2, 4, &mut sc.world, 0, 2);
                }
                add_before_position(pp, &sc.world, 0, &mut sc.positions, o);
                o += 3;
                continue;
            } else if pp > path_length {
                if prev_curve != AFTER {
                    prev_curve = AFTER;
                    vertex.compute_world_vertices(
                        sk,
                        slot,
                        (vertices_length - 6) as usize,
                        4,
                        &mut sc.world,
                        0,
                        2,
                    );
                }
                add_after_position(pp - path_length, &sc.world, 0, &mut sc.positions, o);
                o += 3;
                continue;
            }
            // Determine the curve containing the position.
            loop {
                let length = lengths[curve as usize];
                if pp > length {
                    curve += 1;
                    continue;
                }
                if curve == 0 {
                    pp /= length;
                } else {
                    let prev = lengths[curve as usize - 1];
                    pp = (pp - prev) / (length - prev);
                }
                break;
            }
            if curve != prev_curve {
                prev_curve = curve;
                if closed && curve == curve_count {
                    vertex.compute_world_vertices(
                        sk,
                        slot,
                        (vertices_length - 4) as usize,
                        4,
                        &mut sc.world,
                        0,
                        2,
                    );
                    vertex.compute_world_vertices(sk, slot, 0, 4, &mut sc.world, 4, 2);
                } else {
                    vertex.compute_world_vertices(
                        sk,
                        slot,
                        curve as usize * 6 + 2,
                        8,
                        &mut sc.world,
                        0,
                        2,
                    );
                }
            }
            let w = &sc.world;
            add_curve_position(
                pp,
                w[0],
                w[1],
                w[2],
                w[3],
                w[4],
                w[5],
                w[6],
                w[7],
                &mut sc.positions,
                o,
                tangents || (i > 0 && space < EPSILON),
            );
            o += 3;
        }
        return;
    }

    // World vertices.
    if closed {
        vertices_length += 2;
        sc.world.clear();
        sc.world.resize(vertices_length as usize, 0.0);
        vertex.compute_world_vertices(
            sk,
            slot,
            2,
            (vertices_length - 4) as usize,
            &mut sc.world,
            0,
            2,
        );
        vertex.compute_world_vertices(
            sk,
            slot,
            0,
            2,
            &mut sc.world,
            (vertices_length - 4) as usize,
            2,
        );
        sc.world[(vertices_length - 2) as usize] = sc.world[0];
        sc.world[(vertices_length - 1) as usize] = sc.world[1];
    } else {
        curve_count -= 1;
        vertices_length -= 4;
        sc.world.clear();
        sc.world.resize(vertices_length as usize, 0.0);
        vertex.compute_world_vertices(sk, slot, 2, vertices_length as usize, &mut sc.world, 0, 2);
    }
    let world = &sc.world;

    // Curve lengths.
    sc.curves.clear();
    sc.curves.resize(curve_count as usize, 0.0);
    let mut path_len = 0.0f32;
    let (mut x1, mut y1) = (world[0], world[1]);
    let (mut cx1, mut cy1, mut cx2, mut cy2, mut x2, mut y2) =
        (0.0f32, 0.0f32, 0.0f32, 0.0f32, 0.0f32, 0.0f32);
    let mut w = 2;
    for i in 0..curve_count as usize {
        cx1 = world[w];
        cy1 = world[w + 1];
        cx2 = world[w + 2];
        cy2 = world[w + 3];
        x2 = world[w + 4];
        y2 = world[w + 5];
        let tmpx = (x1 - cx1 * 2.0 + cx2) * 0.1875;
        let tmpy = (y1 - cy1 * 2.0 + cy2) * 0.1875;
        let dddfx = ((cx1 - cx2) * 3.0 - x1 + x2) * 0.09375;
        let dddfy = ((cy1 - cy2) * 3.0 - y1 + y2) * 0.09375;
        let mut ddfx = tmpx * 2.0 + dddfx;
        let mut ddfy = tmpy * 2.0 + dddfy;
        let mut dfx = (cx1 - x1) * 0.75 + tmpx + dddfx * 0.16666667;
        let mut dfy = (cy1 - y1) * 0.75 + tmpy + dddfy * 0.16666667;
        path_len += math::sqrt(dfx * dfx + dfy * dfy);
        dfx += ddfx;
        dfy += ddfy;
        ddfx += dddfx;
        ddfy += dddfy;
        path_len += math::sqrt(dfx * dfx + dfy * dfy);
        dfx += ddfx;
        dfy += ddfy;
        path_len += math::sqrt(dfx * dfx + dfy * dfy);
        dfx += ddfx + dddfx;
        dfy += ddfy + dddfy;
        path_len += math::sqrt(dfx * dfx + dfy * dfy);
        sc.curves[i] = path_len;
        x1 = x2;
        y1 = y2;
        w += 6;
    }
    let path_length = path_len;
    if d.position_mode == PositionMode::Percent {
        position *= path_length;
    }
    let multiplier = match d.spacing_mode {
        SpacingMode::Percent => path_length,
        SpacingMode::Proportional => path_length / spaces_count as f32,
        _ => 1.0,
    };
    let mut curve_length = 0.0f32;
    let mut o = 0;
    let mut curve: i32 = 0;
    let mut segment: usize = 0;
    for i in 0..spaces_count {
        let space = sc.spaces[i] * multiplier;
        position += space;
        let mut pp = position;
        if closed {
            pp = math::fmod(pp, path_length);
            if pp < 0.0 {
                pp += path_length;
            }
            curve = 0;
            segment = 0;
        } else if pp < 0.0 {
            add_before_position(pp, world, 0, &mut sc.positions, o);
            o += 3;
            continue;
        } else if pp > path_length {
            add_after_position(
                pp - path_length,
                world,
                (vertices_length - 4) as usize,
                &mut sc.positions,
                o,
            );
            o += 3;
            continue;
        }
        // Determine the curve containing the position.
        loop {
            let length = sc.curves[curve as usize];
            if pp > length {
                curve += 1;
                continue;
            }
            if curve == 0 {
                pp /= length;
            } else {
                let prev = sc.curves[curve as usize - 1];
                pp = (pp - prev) / (length - prev);
            }
            break;
        }
        // Curve segment lengths.
        if curve != prev_curve {
            prev_curve = curve;
            let ii = curve as usize * 6;
            x1 = world[ii];
            y1 = world[ii + 1];
            cx1 = world[ii + 2];
            cy1 = world[ii + 3];
            cx2 = world[ii + 4];
            cy2 = world[ii + 5];
            x2 = world[ii + 6];
            y2 = world[ii + 7];
            let tmpx = (x1 - cx1 * 2.0 + cx2) * 0.03;
            let tmpy = (y1 - cy1 * 2.0 + cy2) * 0.03;
            let dddfx = ((cx1 - cx2) * 3.0 - x1 + x2) * 0.006;
            let dddfy = ((cy1 - cy2) * 3.0 - y1 + y2) * 0.006;
            let mut ddfx = tmpx * 2.0 + dddfx;
            let mut ddfy = tmpy * 2.0 + dddfy;
            let mut dfx = (cx1 - x1) * 0.3 + tmpx + dddfx * 0.16666667;
            let mut dfy = (cy1 - y1) * 0.3 + tmpy + dddfy * 0.16666667;
            curve_length = math::sqrt(dfx * dfx + dfy * dfy);
            sc.segments[0] = curve_length;
            for seg in 1..8 {
                dfx += ddfx;
                dfy += ddfy;
                ddfx += dddfx;
                ddfy += dddfy;
                curve_length += math::sqrt(dfx * dfx + dfy * dfy);
                sc.segments[seg] = curve_length;
            }
            dfx += ddfx;
            dfy += ddfy;
            curve_length += math::sqrt(dfx * dfx + dfy * dfy);
            sc.segments[8] = curve_length;
            dfx += ddfx + dddfx;
            dfy += ddfy + dddfy;
            curve_length += math::sqrt(dfx * dfx + dfy * dfy);
            sc.segments[9] = curve_length;
            segment = 0;
        }
        // Weight by segment length.
        pp *= curve_length;
        loop {
            let length = sc.segments[segment];
            if pp > length {
                segment += 1;
                continue;
            }
            if segment == 0 {
                pp /= length;
            } else {
                let prev = sc.segments[segment - 1];
                pp = segment as f32 + (pp - prev) / (length - prev);
            }
            break;
        }
        add_curve_position(
            pp * 0.1,
            x1,
            y1,
            cx1,
            cy1,
            cx2,
            cy2,
            x2,
            y2,
            &mut sc.positions,
            o,
            tangents || (i > 0 && space < EPSILON),
        );
        o += 3;
    }
}

fn add_before_position(p: f32, temp: &[f32], i: usize, output: &mut [f32], o: usize) {
    let (x1, y1) = (temp[i], temp[i + 1]);
    let (dx, dy) = (temp[i + 2] - x1, temp[i + 3] - y1);
    let r = math::atan2(dy, dx);
    output[o] = x1 + p * math::cos(r);
    output[o + 1] = y1 + p * math::sin(r);
    output[o + 2] = r;
}

fn add_after_position(p: f32, temp: &[f32], i: usize, output: &mut [f32], o: usize) {
    let (x1, y1) = (temp[i + 2], temp[i + 3]);
    let (dx, dy) = (x1 - temp[i], y1 - temp[i + 1]);
    let r = math::atan2(dy, dx);
    output[o] = x1 + p * math::cos(r);
    output[o + 1] = y1 + p * math::sin(r);
    output[o + 2] = r;
}

#[allow(clippy::too_many_arguments)]
fn add_curve_position(
    p: f32,
    x1: f32,
    y1: f32,
    cx1: f32,
    cy1: f32,
    cx2: f32,
    cy2: f32,
    x2: f32,
    y2: f32,
    output: &mut [f32],
    o: usize,
    tangents: bool,
) {
    if p < EPSILON || p.is_nan() {
        output[o] = x1;
        output[o + 1] = y1;
        output[o + 2] = math::atan2(cy1 - y1, cx1 - x1);
        return;
    }
    let tt = p * p;
    let ttt = tt * p;
    let u = 1.0 - p;
    let uu = u * u;
    let uuu = uu * u;
    let ut = u * p;
    let ut3 = ut * 3.0;
    let uut3 = u * ut3;
    let utt3 = ut3 * p;
    let x = x1 * uuu + cx1 * uut3 + cx2 * utt3 + x2 * ttt;
    let y = y1 * uuu + cy1 * uut3 + cy2 * utt3 + y2 * ttt;
    output[o] = x;
    output[o + 1] = y;
    if tangents {
        if p < 0.001 {
            output[o + 2] = math::atan2(cy1 - y1, cx1 - x1);
        } else {
            output[o + 2] = math::atan2(
                y - (y1 * uu + cy1 * ut * 2.0 + cy2 * tt),
                x - (x1 * uu + cx1 * ut * 2.0 + cx2 * tt),
            );
        }
    }
}

pub(super) fn sort(sk: &mut Skeleton, data: &SkeletonData, index: usize) {
    let ConstraintData::Path(d) = &data.constraints[index] else {
        return;
    };
    let slot = parts(&sk.constraints[index]).slot;
    let slot_bone = sk.slots[slot.0].bone;
    let skin = sk.skin.clone();
    let default_skin = sk.data.default_skin.clone();
    if let Some(skin) = &skin {
        sort_path_slot(sk, skin, slot.0, slot_bone);
    }
    if let Some(default_skin) = &default_skin {
        if skin
            .as_ref()
            .is_none_or(|s| !std::sync::Arc::ptr_eq(s, default_skin))
        {
            sort_path_slot(sk, default_skin, slot.0, slot_bone);
        }
    }
    let current = sk.slots[slot.0].pose.attachment.clone();
    sort_path(sk, current.as_deref(), slot_bone);
    for &b in &d.bones {
        sk.sort_bone(BoneId(b));
        sk.constrained_bone(BoneId(b));
    }
    sk.update_cache.push(Update::Constraint(index));
    for &b in &d.bones {
        sk.sort_reset_children(BoneId(b));
    }
    for &b in &d.bones {
        sk.bones[b].sorted = true;
    }
}

fn sort_path_slot(sk: &mut Skeleton, skin: &Skin, slot_index: usize, slot_bone: BoneId) {
    for entry in skin.entries_for_slot(slot_index) {
        sort_path(sk, Some(&entry.attachment), slot_bone);
    }
}

fn sort_path(sk: &mut Skeleton, attachment: Option<&Attachment>, slot_bone: BoneId) {
    let Some(Attachment::Path(path)) = attachment else {
        return;
    };
    let path_bones = &path.vertex.bones;
    if path_bones.is_empty() {
        sk.sort_bone(slot_bone);
    } else {
        let mut i = 0;
        while i < path_bones.len() {
            let n = path_bones[i];
            i += 1;
            let end = i + n;
            while i < end {
                sk.sort_bone(BoneId(path_bones[i]));
                i += 1;
            }
        }
    }
}
