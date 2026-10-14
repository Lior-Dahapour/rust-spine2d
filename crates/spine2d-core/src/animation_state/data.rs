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

use crate::skeleton_data::SkeletonData;
use rustc_hash::FxHashMap;
use smol_str::SmolStr;
use std::sync::Arc;

/// Stores mix (crossfade) durations to be applied when `AnimationState` animations are changed.
#[derive(Clone, Debug)]
pub struct AnimationStateData {
    skeleton_data: Arc<SkeletonData>,
    /// The mix duration used when no specific mix is set for a pair of animations.
    pub default_mix: f32,
    mixes: FxHashMap<SmolStr, FxHashMap<SmolStr, f32>>,
}

impl AnimationStateData {
    pub fn new(skeleton_data: Arc<SkeletonData>) -> Self {
        AnimationStateData {
            skeleton_data,
            default_mix: 0.0,
            mixes: FxHashMap::default(),
        }
    }

    pub fn skeleton_data(&self) -> &Arc<SkeletonData> {
        &self.skeleton_data
    }

    /// Sets the mix duration when changing from `from` to `to` animation. Returns false if
    /// either animation does not exist.
    pub fn set_mix(&mut self, from: &str, to: &str, duration: f32) -> bool {
        if self.skeleton_data.find_animation(from).is_none()
            || self.skeleton_data.find_animation(to).is_none()
        {
            return false;
        }
        self.mixes
            .entry(from.into())
            .or_default()
            .insert(to.into(), duration);
        true
    }

    /// The mix duration for changing from `from` to `to`, or the default mix.
    pub fn mix(&self, from: &str, to: &str) -> f32 {
        self.mixes
            .get(from)
            .and_then(|m| m.get(to))
            .copied()
            .unwrap_or(self.default_mix)
    }

    /// Removes all mixes and resets the default mix to 0.
    pub fn clear(&mut self) {
        self.default_mix = 0.0;
        self.mixes.clear();
    }
}
