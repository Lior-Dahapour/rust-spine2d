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

/// What a method panics with when it needs the data a detached one does not hold.
const DETACHED: &str =
    "these mix settings hold no data; use the `_with` methods (see `AnimationStateData::detached`)";

/// Stores mix (crossfade) durations to be applied when `AnimationState` animations are changed.
#[derive(Clone, Debug)]
pub struct AnimationStateData {
    /// `None` for a [`AnimationStateData::detached`] one, and while `set_mix` has it moved out.
    skeleton_data: Option<Arc<SkeletonData>>,
    /// The mix duration used when no specific mix is set for a pair of animations.
    pub default_mix: f32,
    mixes: FxHashMap<SmolStr, FxHashMap<SmolStr, f32>>,
}

impl AnimationStateData {
    /// Mix settings that hold a share of `skeleton_data`. This is the one to use, unless you are
    /// building a runtime: see `AnimationStateData::detached`, which needs the `external-data`
    /// feature.
    pub fn new(skeleton_data: Arc<SkeletonData>) -> Self {
        AnimationStateData {
            skeleton_data: Some(skeleton_data),
            default_mix: 0.0,
            mixes: FxHashMap::default(),
        }
    }

    /// Mix settings that hold no [`SkeletonData`]: whoever calls it keeps the data and passes it
    /// to `set_mix_with`.
    ///
    /// Needs the `external-data` feature.
    ///
    /// **This is for building a runtime or something similar**, where the data lives in one place
    /// and many skeletons use it. Anyone else should use [`AnimationStateData::new`], which holds
    /// its data and needs none of this. The methods without `_with`, and
    /// [`AnimationStateData::skeleton_data`], panic on a detached one because they have no data to
    /// read, so always pass the same data the skeleton was made from. To start an animation by
    /// name, find it in the data ([`SkeletonData::find_animation`]) and pass it to
    /// `AnimationState::set_animation`.
    #[cfg_attr(not(feature = "external-data"), visibility::make(pub(crate)))]
    #[cfg_attr(not(feature = "external-data"), allow(dead_code))]
    pub fn detached() -> Self {
        AnimationStateData {
            skeleton_data: None,
            default_mix: 0.0,
            mixes: FxHashMap::default(),
        }
    }

    /// The data these settings hold. Panics for a detached one (the `external-data` feature).
    pub fn skeleton_data(&self) -> &Arc<SkeletonData> {
        self.skeleton_data.as_ref().expect(DETACHED)
    }

    /// Sets the mix duration when changing from `from` to `to` animation. Returns false if
    /// either animation does not exist.
    pub fn set_mix(&mut self, from: &str, to: &str, duration: f32) -> bool {
        // The data is moved out and back, because `set_mix_with` needs `&mut self` and the data.
        let skeleton_data = self.skeleton_data.take().expect(DETACHED);
        let set = self.set_mix_with(&skeleton_data, from, to, duration);
        self.skeleton_data = Some(skeleton_data);
        set
    }

    /// Like [`AnimationStateData::set_mix`], with the data given instead of held.
    #[cfg_attr(not(feature = "external-data"), visibility::make(pub(crate)))]
    pub fn set_mix_with(
        &mut self,
        skeleton_data: &SkeletonData,
        from: &str,
        to: &str,
        duration: f32,
    ) -> bool {
        if skeleton_data.find_animation(from).is_none()
            || skeleton_data.find_animation(to).is_none()
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
