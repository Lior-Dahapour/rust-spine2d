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

use crate::slot::SlotPose;
use crate::texture_region::TextureRegion;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;

static NEXT_ID: AtomicU32 = AtomicU32::new(0);

/// A frame sequence for a region or mesh attachment: one texture region per frame, with the UVs
/// (and, for regions, vertex offsets) precomputed by `update_*`.
#[derive(Debug)]
pub struct Sequence {
    id: u32,
    pub regions: Vec<Option<Arc<TextureRegion>>>,
    path_suffix: bool,
    uvs: Vec<Vec<f32>>,
    offsets: Vec<Vec<f32>>,
    pub start: i32,
    pub digits: i32,
    pub setup_index: i32,
}

impl Clone for Sequence {
    /// Like the reference copy constructor: a copy gets a new id.
    fn clone(&self) -> Self {
        Sequence {
            id: NEXT_ID.fetch_add(1, Ordering::Relaxed),
            regions: self.regions.clone(),
            path_suffix: self.path_suffix,
            uvs: self.uvs.clone(),
            offsets: self.offsets.clone(),
            start: self.start,
            digits: self.digits,
            setup_index: self.setup_index,
        }
    }
}

impl Sequence {
    pub fn new(count: usize, path_suffix: bool) -> Sequence {
        Sequence {
            id: NEXT_ID.fetch_add(1, Ordering::Relaxed),
            regions: vec![None; count],
            path_suffix,
            uvs: Vec::new(),
            offsets: Vec::new(),
            start: 0,
            digits: 0,
            setup_index: 0,
        }
    }

    pub fn id(&self) -> u32 {
        self.id
    }

    pub fn has_path_suffix(&self) -> bool {
        self.path_suffix
    }

    /// Recomputes UVs and vertex offsets for a region attachment; `f` fills `(offsets, uvs)`
    /// (8 floats each) for one frame.
    pub(crate) fn update_region(
        &mut self,
        mut f: impl FnMut(Option<&TextureRegion>, &mut [f32], &mut [f32]),
    ) {
        let n = self.regions.len();
        self.uvs.resize(n, Vec::new());
        self.offsets.resize(n, Vec::new());
        for i in 0..n {
            self.uvs[i] = vec![0.0; 8];
            self.offsets[i] = vec![0.0; 8];
            f(
                self.regions[i].as_deref(),
                &mut self.offsets[i],
                &mut self.uvs[i],
            );
        }
    }

    /// Recomputes UVs for a mesh attachment (`uv_len` floats per frame); offsets are not used.
    pub(crate) fn update_mesh(
        &mut self,
        uv_len: usize,
        mut f: impl FnMut(Option<&TextureRegion>, &mut [f32]),
    ) {
        let n = self.regions.len();
        self.uvs.resize(n, Vec::new());
        self.offsets.clear();
        for i in 0..n {
            self.uvs[i] = vec![0.0; uv_len];
            f(self.regions[i].as_deref(), &mut self.uvs[i]);
        }
    }

    /// The frame index to use for a slot pose, clamped to the available frames.
    pub fn resolve_index(&self, pose: &SlotPose) -> usize {
        let mut index = pose.sequence_index;
        if index == -1 {
            index = self.setup_index;
        }
        index.clamp(0, self.regions.len() as i32 - 1).max(0) as usize
    }

    pub fn region(&self, index: usize) -> Option<&Arc<TextureRegion>> {
        self.regions[index].as_ref()
    }

    pub fn uvs(&self, index: usize) -> &[f32] {
        &self.uvs[index]
    }

    pub fn offsets(&self, index: usize) -> &[f32] {
        &self.offsets[index]
    }

    /// The atlas path of a frame: `base_path` plus the zero-padded frame number if the sequence
    /// uses path suffixes.
    pub fn path(&self, base_path: &str, index: i32) -> String {
        if !self.path_suffix {
            return base_path.to_string();
        }
        let frame = (self.start + index).to_string();
        let pad = (self.digits - frame.len() as i32).max(0) as usize;
        format!("{base_path}{}{frame}", "0".repeat(pad))
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum SequenceMode {
    #[default]
    Hold = 0,
    Once = 1,
    Loop = 2,
    Pingpong = 3,
    OnceReverse = 4,
    LoopReverse = 5,
    PingpongReverse = 6,
}

impl SequenceMode {
    pub fn from_name(name: &str) -> SequenceMode {
        match name {
            "once" => SequenceMode::Once,
            "loop" => SequenceMode::Loop,
            "pingpong" => SequenceMode::Pingpong,
            "onceReverse" => SequenceMode::OnceReverse,
            "loopReverse" => SequenceMode::LoopReverse,
            "pingpongReverse" => SequenceMode::PingpongReverse,
            _ => SequenceMode::Hold,
        }
    }

    /// From the binary format's index; out-of-range values are `Hold`.
    pub fn from_index(index: u8) -> SequenceMode {
        match index {
            1 => SequenceMode::Once,
            2 => SequenceMode::Loop,
            3 => SequenceMode::Pingpong,
            4 => SequenceMode::OnceReverse,
            5 => SequenceMode::LoopReverse,
            6 => SequenceMode::PingpongReverse,
            _ => SequenceMode::Hold,
        }
    }
}
