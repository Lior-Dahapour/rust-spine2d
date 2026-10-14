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

use super::curve::{CurveTimeline, BEZIER, BEZIER_SIZE, LINEAR, STEPPED};
use super::{search, search_step, ApplyOptions, MixFrom, Property, PropertyId};
use crate::attachment::{Attachment, VertexAttachment};
use crate::sequence::SequenceMode;
use crate::skeleton::Skeleton;
use crate::slot::SlotPose;
use smallvec::{smallvec, SmallVec};
use smol_str::SmolStr;
use std::sync::Arc;

/// Sets a slot's attachment by placeholder name at discrete times.
#[derive(Clone, Debug)]
pub struct AttachmentTimeline {
    pub frames: Vec<f32>,
    /// Placeholder name per frame; empty clears the attachment.
    pub attachment_names: Vec<SmolStr>,
    pub slot: usize,
}

impl AttachmentTimeline {
    pub fn new(frame_count: usize, slot: usize) -> Self {
        AttachmentTimeline {
            frames: vec![0.0; frame_count],
            attachment_names: vec![SmolStr::default(); frame_count],
            slot,
        }
    }

    pub fn set_frame(&mut self, frame: usize, time: f32, attachment_name: &str) {
        self.frames[frame] = time;
        self.attachment_names[frame] = attachment_name.into();
    }

    pub(crate) fn property_ids(&self) -> SmallVec<[PropertyId; 3]> {
        smallvec![Property::Attachment.id(self.slot)]
    }

    pub(crate) fn apply(&self, skeleton: &mut Skeleton, time: f32, o: &ApplyOptions) {
        if !skeleton.bones[skeleton.slots[self.slot].bone.0].active {
            return;
        }
        let attachment = if o.out || time < self.frames[0] {
            if o.from == MixFrom::Current {
                return;
            }
            skeleton.attachment_by_index(self.slot, &skeleton.data.slots[self.slot].attachment_name)
        } else {
            skeleton.attachment_by_index(
                self.slot,
                &self.attachment_names[search(&self.frames, time)],
            )
        };
        let slot = &mut skeleton.slots[self.slot];
        let pose = if o.applied_pose {
            slot.applied_pose_mut()
        } else {
            &mut slot.pose
        };
        pose.set_attachment(attachment);
    }
}

/// Changes a vertex attachment's deform (vertex offsets) over time.
#[derive(Clone, Debug)]
pub struct DeformTimeline {
    pub curve: CurveTimeline,
    pub slot: usize,
    /// The attachment whose timelines this is; slots showing it (or attachments linked to it)
    /// are deformed.
    pub attachment: Arc<Attachment>,
    /// Deform vertices per frame; each has the same length.
    pub vertices: Vec<Vec<f32>>,
}

impl DeformTimeline {
    pub fn new(
        frame_count: usize,
        bezier_count: usize,
        slot: usize,
        attachment: Arc<Attachment>,
    ) -> Self {
        DeformTimeline {
            curve: CurveTimeline::new(frame_count, 1, bezier_count),
            slot,
            attachment,
            vertices: vec![Vec::new(); frame_count],
        }
    }

    pub fn set_frame(&mut self, frame: usize, time: f32, vertices: &[f32]) {
        self.curve.frames[frame] = time;
        self.vertices[frame].clear();
        self.vertices[frame].extend_from_slice(vertices);
    }

    pub(crate) fn property_ids(&self) -> SmallVec<[PropertyId; 3]> {
        let id = (self.slot << 16 | self.attachment.id().0 as usize) & 0xffff_ffff;
        smallvec![Property::Deform.id(id)]
    }

    /// Like `CurveTimeline::set_bezier` but the y values are percents (0..1) of the way between
    /// the frame's vertices.
    #[allow(clippy::too_many_arguments)]
    pub fn set_bezier(
        &mut self,
        bezier: usize,
        frame: usize,
        value: usize,
        time1: f32,
        _value1: f32,
        cx1: f32,
        cy1: f32,
        cx2: f32,
        cy2: f32,
        time2: f32,
        _value2: f32,
    ) {
        let mut i = self.curve.frame_count() + bezier * BEZIER_SIZE;
        let curves = &mut self.curve.curves;
        if value == 0 {
            curves[frame] = BEZIER + i as f32;
        }
        let tmpx = (time1 - cx1 * 2.0 + cx2) * 0.03;
        let tmpy = cy2 * 0.03 - cy1 * 0.06;
        let dddx = ((cx1 - cx2) * 3.0 - time1 + time2) * 0.006;
        let dddy = (cy1 - cy2 + 0.33333333) * 0.018;
        let mut ddx = tmpx * 2.0 + dddx;
        let mut ddy = tmpy * 2.0 + dddy;
        let mut dx = (cx1 - time1) * 0.3 + tmpx + dddx * 0.16666667;
        let mut dy = cy1 * 0.3 + tmpy + dddy * 0.16666667;
        let mut x = time1 + dx;
        let mut y = dy;
        let n = i + BEZIER_SIZE;
        while i < n {
            curves[i] = x;
            curves[i + 1] = y;
            dx += ddx;
            dy += ddy;
            ddx += dddx;
            ddy += dddy;
            x += dx;
            y += dy;
            i += 2;
        }
    }

    /// The interpolation percent (0..1) between `frame` and the next frame at `time`.
    pub fn curve_percent(&self, time: f32, frame: usize) -> f32 {
        let curves = &self.curve.curves;
        let frames = &self.curve.frames;
        let t = curves[frame];
        if t == LINEAR {
            let x = frames[frame];
            return (time - x) / (frames[frame + 1] - x);
        }
        if t == STEPPED {
            return 0.0;
        }
        let mut i = t as usize - BEZIER as usize;
        if curves[i] > time {
            let x = frames[frame];
            return curves[i + 1] * (time - x) / (curves[i] - x);
        }
        let n = i + BEZIER_SIZE;
        i += 2;
        while i < n {
            if curves[i] >= time {
                let x = curves[i - 2];
                let y = curves[i - 1];
                return y + (time - x) / (curves[i] - x) * (curves[i + 1] - y);
            }
            i += 2;
        }
        let x = curves[n - 2];
        let y = curves[n - 1];
        y + (1.0 - y) * (time - x) / (frames[frame + 1] - x)
    }

    pub(crate) fn apply(&self, skeleton: &mut Skeleton, time: f32, o: &ApplyOptions) {
        if !self
            .attachment
            .is_timeline_active(skeleton, self.slot, o.applied_pose)
        {
            return;
        }
        let timeline_slots = self.attachment.base().timeline_slots.clone();
        let frames = &self.curve.frames;

        if time < frames[0] {
            self.apply_before_first(skeleton, self.slot, o);
            for &s in &timeline_slots {
                self.apply_before_first(skeleton, s, o);
            }
            return;
        }

        let (v1, v2, percent);
        if time >= frames[frames.len() - 1] {
            v1 = &self.vertices[frames.len() - 1];
            v2 = None;
            percent = 0.0;
        } else {
            let frame = search(frames, time);
            percent = self.curve_percent(time, frame);
            v1 = &self.vertices[frame];
            v2 = Some(&self.vertices[frame + 1]);
        }
        let vertex_count = self.vertices[0].len();
        self.apply_to_slot(skeleton, self.slot, o, v1, v2, percent, vertex_count);
        for &s in &timeline_slots {
            self.apply_to_slot(skeleton, s, o, v1, v2, percent, vertex_count);
        }
    }

    /// Returns the slot's pose split into the pieces deform needs, if the slot is active and
    /// shows an attachment using this timeline.
    fn target<'a>(
        &self,
        skeleton: &'a mut Skeleton,
        slot_index: usize,
        applied_pose: bool,
    ) -> Option<(&'a mut Vec<f32>, &'a VertexAttachment)> {
        let slot = &mut skeleton.slots[slot_index];
        if !skeleton.bones[slot.bone.0].active {
            return None;
        }
        let SlotPose {
            attachment, deform, ..
        } = if applied_pose {
            slot.applied_pose_mut()
        } else {
            &mut slot.pose
        };
        let attachment = attachment.as_deref()?;
        if attachment.timeline_id() != self.attachment.id() {
            return None;
        }
        Some((deform, attachment.as_vertex()?))
    }

    fn apply_before_first(&self, skeleton: &mut Skeleton, slot_index: usize, o: &ApplyOptions) {
        let Some((deform, vertex)) = self.target(skeleton, slot_index, o.applied_pose) else {
            return;
        };
        let mut from = o.from;
        if deform.is_empty() {
            from = MixFrom::Setup;
        }
        match from {
            MixFrom::Setup => deform.clear(),
            MixFrom::First => {
                if o.alpha == 1.0 {
                    deform.clear();
                    return;
                }
                let vertex_count = self.vertices[0].len();
                deform.resize(vertex_count, 0.0);
                if vertex.bones.is_empty() {
                    for (d, s) in deform.iter_mut().zip(&vertex.vertices) {
                        *d += (s - *d) * o.alpha;
                    }
                } else {
                    let a = 1.0 - o.alpha;
                    for d in deform.iter_mut() {
                        *d *= a;
                    }
                }
            }
            MixFrom::Current => {}
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn apply_to_slot(
        &self,
        skeleton: &mut Skeleton,
        slot_index: usize,
        o: &ApplyOptions,
        v1: &[f32],
        v2: Option<&Vec<f32>>,
        percent: f32,
        vertex_count: usize,
    ) {
        let Some((deform, vertex)) = self.target(skeleton, slot_index, o.applied_pose) else {
            return;
        };
        Self::apply_to_pose(
            deform,
            vertex,
            v1,
            v2.map(|v| v.as_slice()),
            percent,
            vertex_count,
            o,
        );
    }

    #[allow(clippy::too_many_arguments)]
    fn apply_to_pose(
        deform: &mut Vec<f32>,
        vertex: &VertexAttachment,
        v1: &[f32],
        v2: Option<&[f32]>,
        percent: f32,
        vertex_count: usize,
        o: &ApplyOptions,
    ) {
        let (alpha, add) = (o.alpha, o.add);
        let mut from = o.from;
        if deform.is_empty() {
            from = MixFrom::Setup;
        }
        let from_setup = from == MixFrom::Setup;
        deform.resize(vertex_count, 0.0);
        let deform = &mut deform[..vertex_count];
        let v1 = &v1[..vertex_count];
        let unweighted = vertex.bones.is_empty();
        let setup = &vertex.vertices[..if unweighted { vertex_count } else { 0 }];

        let Some(v2) = v2 else {
            // Single key (at or after the last frame).
            if alpha == 1.0 {
                if add && !from_setup {
                    if unweighted {
                        for i in 0..vertex_count {
                            deform[i] += v1[i] - setup[i];
                        }
                    } else {
                        for i in 0..vertex_count {
                            deform[i] += v1[i];
                        }
                    }
                } else {
                    deform.copy_from_slice(v1);
                }
            } else if from_setup {
                if unweighted {
                    for i in 0..vertex_count {
                        let s = setup[i];
                        deform[i] = s + (v1[i] - s) * alpha;
                    }
                } else {
                    for i in 0..vertex_count {
                        deform[i] = v1[i] * alpha;
                    }
                }
            } else if add {
                if unweighted {
                    for i in 0..vertex_count {
                        deform[i] += (v1[i] - setup[i]) * alpha;
                    }
                } else {
                    for i in 0..vertex_count {
                        deform[i] += v1[i] * alpha;
                    }
                }
            } else {
                for i in 0..vertex_count {
                    deform[i] += (v1[i] - deform[i]) * alpha;
                }
            }
            return;
        };

        let v2 = &v2[..vertex_count];
        if alpha == 1.0 {
            if add && !from_setup {
                if unweighted {
                    for i in 0..vertex_count {
                        let prev = v1[i];
                        deform[i] += prev + (v2[i] - prev) * percent - setup[i];
                    }
                } else {
                    for i in 0..vertex_count {
                        let prev = v1[i];
                        deform[i] += prev + (v2[i] - prev) * percent;
                    }
                }
            } else if percent == 0.0 {
                deform.copy_from_slice(v1);
            } else {
                for i in 0..vertex_count {
                    let prev = v1[i];
                    deform[i] = prev + (v2[i] - prev) * percent;
                }
            }
        } else if from_setup {
            if unweighted {
                for i in 0..vertex_count {
                    let (prev, s) = (v1[i], setup[i]);
                    deform[i] = s + (prev + (v2[i] - prev) * percent - s) * alpha;
                }
            } else {
                for i in 0..vertex_count {
                    let prev = v1[i];
                    deform[i] = (prev + (v2[i] - prev) * percent) * alpha;
                }
            }
        } else if add {
            if unweighted {
                for i in 0..vertex_count {
                    let prev = v1[i];
                    deform[i] += (prev + (v2[i] - prev) * percent - setup[i]) * alpha;
                }
            } else {
                for i in 0..vertex_count {
                    let prev = v1[i];
                    deform[i] += (prev + (v2[i] - prev) * percent) * alpha;
                }
            }
        } else {
            for i in 0..vertex_count {
                let prev = v1[i];
                deform[i] += (prev + (v2[i] - prev) * percent - deform[i]) * alpha;
            }
        }
    }
}

/// Changes the frame of a region or mesh attachment's sequence at discrete times.
#[derive(Clone, Debug)]
pub struct SequenceTimeline {
    pub frames: Vec<f32>,
    pub slot: usize,
    pub attachment: Arc<Attachment>,
}

impl SequenceTimeline {
    const ENTRIES: usize = 3;
    const MODE: usize = 1;
    const DELAY: usize = 2;

    pub fn new(frame_count: usize, slot: usize, attachment: Arc<Attachment>) -> Self {
        SequenceTimeline {
            frames: vec![0.0; frame_count * Self::ENTRIES],
            slot,
            attachment,
        }
    }

    pub fn set_frame(
        &mut self,
        frame: usize,
        time: f32,
        mode: SequenceMode,
        index: i32,
        delay: f32,
    ) {
        let i = frame * Self::ENTRIES;
        self.frames[i] = time;
        self.frames[i + Self::MODE] = (mode as i32 | (index << 4)) as f32;
        self.frames[i + Self::DELAY] = delay;
    }

    fn sequence(&self) -> Option<&crate::sequence::Sequence> {
        match &*self.attachment {
            Attachment::Region(a) => Some(&a.sequence),
            Attachment::Mesh(a) => Some(&a.sequence),
            _ => None,
        }
    }

    pub(crate) fn property_ids(&self) -> SmallVec<[PropertyId; 3]> {
        let sequence_id = self.sequence().map_or(0, |s| s.id() as usize);
        let id = (self.slot << 16 | sequence_id) & 0xffff_ffff;
        smallvec![Property::Sequence.id(id)]
    }

    pub(crate) fn apply(&self, skeleton: &mut Skeleton, time: f32, o: &ApplyOptions) {
        if !self
            .attachment
            .is_timeline_active(skeleton, self.slot, o.applied_pose)
        {
            return;
        }
        let Some(sequence) = self.sequence() else {
            return;
        };
        let timeline_slots = self.attachment.base().timeline_slots.clone();

        if o.out || time < self.frames[0] {
            if o.from != MixFrom::Current {
                self.reset_slot(skeleton, self.slot, o.applied_pose);
                for &s in &timeline_slots {
                    self.reset_slot(skeleton, s, o.applied_pose);
                }
            }
            return;
        }
        let i = search_step(&self.frames, time, Self::ENTRIES);
        let before = self.frames[i];
        let mode_and_index = self.frames[i + Self::MODE] as i32;
        let delay = self.frames[i + Self::DELAY];
        let count = sequence.regions.len() as i32;
        self.apply_to_slot(
            skeleton,
            self.slot,
            o.applied_pose,
            count,
            time,
            before,
            mode_and_index,
            delay,
        );
        for &s in &timeline_slots {
            self.apply_to_slot(
                skeleton,
                s,
                o.applied_pose,
                count,
                time,
                before,
                mode_and_index,
                delay,
            );
        }
    }

    /// The pose of an active slot showing an attachment that uses this timeline.
    fn target<'a>(
        &self,
        skeleton: &'a mut Skeleton,
        slot_index: usize,
        applied_pose: bool,
    ) -> Option<&'a mut SlotPose> {
        let slot = &mut skeleton.slots[slot_index];
        if !skeleton.bones[slot.bone.0].active {
            return None;
        }
        let pose = if applied_pose {
            slot.applied_pose_mut()
        } else {
            &mut slot.pose
        };
        match &pose.attachment {
            Some(a) if a.timeline_id() == self.attachment.id() => Some(pose),
            _ => None,
        }
    }

    fn reset_slot(&self, skeleton: &mut Skeleton, slot_index: usize, applied_pose: bool) {
        if let Some(pose) = self.target(skeleton, slot_index, applied_pose) {
            pose.sequence_index = -1;
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn apply_to_slot(
        &self,
        skeleton: &mut Skeleton,
        slot_index: usize,
        applied_pose: bool,
        count: i32,
        time: f32,
        before: f32,
        mode_and_index: i32,
        delay: f32,
    ) {
        let Some(pose) = self.target(skeleton, slot_index, applied_pose) else {
            return;
        };
        let mut index = mode_and_index >> 4;
        let mode = SequenceMode::from_index((mode_and_index & 0xf) as u8);
        if mode != SequenceMode::Hold {
            index += ((time - before) / delay + 0.0001) as i32;
            match mode {
                SequenceMode::Once => index = (count - 1).min(index),
                SequenceMode::Loop => index %= count,
                SequenceMode::Pingpong => {
                    let n = (count << 1) - 2;
                    index = if n == 0 { 0 } else { index % n };
                    if index >= count {
                        index = n - index;
                    }
                }
                SequenceMode::OnceReverse => index = (count - 1 - index).max(0),
                SequenceMode::LoopReverse => index = count - 1 - (index % count),
                SequenceMode::PingpongReverse => {
                    let n = (count << 1) - 2;
                    index = if n == 0 { 0 } else { (index + count - 1) % n };
                    if index >= count {
                        index = n - index;
                    }
                }
                SequenceMode::Hold => {}
            }
        }
        pose.sequence_index = index;
    }
}
