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

use super::{search, ApplyOptions, MixFrom, Property, PropertyId};
use crate::skeleton::Skeleton;
use crate::slot::SlotId;
use smallvec::{smallvec, SmallVec};

/// Changes the draw order of all slots at discrete times.
#[derive(Clone, Debug)]
pub struct DrawOrderTimeline {
    pub frames: Vec<f32>,
    /// Per frame, for each draw order position the setup index of the slot there; empty means
    /// the setup draw order.
    pub draw_orders: Vec<Vec<usize>>,
}

impl DrawOrderTimeline {
    pub fn new(frame_count: usize) -> Self {
        DrawOrderTimeline {
            frames: vec![0.0; frame_count],
            draw_orders: vec![Vec::new(); frame_count],
        }
    }

    pub fn property_id() -> PropertyId {
        Property::DrawOrder.id(0)
    }

    pub fn set_frame(&mut self, frame: usize, time: f32, draw_order: Option<&[usize]>) {
        self.frames[frame] = time;
        self.draw_orders[frame].clear();
        if let Some(order) = draw_order {
            self.draw_orders[frame].extend_from_slice(order);
        }
    }

    pub(crate) fn property_ids(&self) -> SmallVec<[PropertyId; 3]> {
        smallvec![Self::property_id()]
    }

    pub(crate) fn apply(&self, skeleton: &mut Skeleton, time: f32, o: &ApplyOptions) {
        let slot_count = skeleton.slots.len();
        let pose = skeleton.draw_order.pose_vec(o.applied_pose);
        let reset = |pose: &mut Vec<SlotId>| {
            pose.clear();
            pose.extend((0..slot_count).map(SlotId));
        };
        if o.out || time < self.frames[0] {
            if o.from != MixFrom::Current {
                reset(pose);
            }
            return;
        }
        let order = &self.draw_orders[search(&self.frames, time)];
        if order.is_empty() {
            reset(pose);
        } else {
            pose.clear();
            pose.extend(order.iter().map(|&i| SlotId(i)));
        }
    }
}

/// Changes the draw order of a subset of slots (a folder), leaving the others where they are.
#[derive(Clone, Debug)]
pub struct DrawOrderFolderTimeline {
    pub frames: Vec<f32>,
    /// Setup indices of the slots in the folder.
    slots: Vec<usize>,
    in_folder: Vec<bool>,
    /// Per frame, the folder-relative order; empty means the setup order.
    pub draw_orders: Vec<Vec<usize>>,
}

impl DrawOrderFolderTimeline {
    pub fn new(frame_count: usize, slots: Vec<usize>, slot_count: usize) -> Self {
        let mut in_folder = vec![false; slot_count];
        for &s in &slots {
            in_folder[s] = true;
        }
        DrawOrderFolderTimeline {
            frames: vec![0.0; frame_count],
            slots,
            in_folder,
            draw_orders: vec![Vec::new(); frame_count],
        }
    }

    pub fn slots(&self) -> &[usize] {
        &self.slots
    }

    pub fn set_frame(&mut self, frame: usize, time: f32, draw_order: Option<&[usize]>) {
        self.frames[frame] = time;
        self.draw_orders[frame].clear();
        if let Some(order) = draw_order {
            self.draw_orders[frame].extend_from_slice(order);
        }
    }

    pub(crate) fn property_ids(&self) -> SmallVec<[PropertyId; 3]> {
        self.slots
            .iter()
            .map(|&s| Property::DrawOrderFolder.id(s))
            .collect()
    }

    pub(crate) fn apply(&self, skeleton: &mut Skeleton, time: f32, o: &ApplyOptions) {
        let pose = skeleton.draw_order.pose_vec(o.applied_pose);
        if o.out || time < self.frames[0] {
            if o.from != MixFrom::Current {
                self.place(pose, None);
            }
        } else {
            let order = &self.draw_orders[search(&self.frames, time)];
            self.place(pose, if order.is_empty() { None } else { Some(order) });
        }
    }

    /// Rewrites the folder's positions in `pose`: the n-th folder position takes the slot
    /// `slots[order[n]]` (or `slots[n]` for the setup order).
    fn place(&self, pose: &mut [SlotId], order: Option<&[usize]>) {
        if self.slots.is_empty() {
            return;
        }
        let mut found = 0;
        for entry in pose.iter_mut() {
            if self.in_folder[entry.0] {
                let which = order.map_or(found, |o| o[found]);
                *entry = SlotId(self.slots[which]);
                found += 1;
                if found == self.slots.len() {
                    break;
                }
            }
        }
    }
}
