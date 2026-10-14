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

use crate::animation::Animation;
use crate::bone::BoneData;
use crate::constraint_data::ConstraintData;
use crate::event::EventData;
use crate::skin::Skin;
use crate::slot::SlotData;
use smol_str::SmolStr;
use std::sync::Arc;

/// Immutable (after loading) setup data shared by every [`Skeleton`](crate::Skeleton) instance.
///
/// Constraints, animations, events and the other pieces are added as they are ported.
#[derive(Clone, Debug, Default)]
pub struct SkeletonData {
    pub name: SmolStr,
    pub bones: Vec<BoneData>,
    pub slots: Vec<SlotData>,
    /// All skins, including the default skin.
    pub skins: Vec<Arc<Skin>>,
    /// The skin holding the setup pose attachments; also present in `skins`.
    pub default_skin: Option<Arc<Skin>>,
    pub x: f32,
    pub y: f32,
    /// All constraints in application order.
    pub constraints: Vec<ConstraintData>,
    pub events: Vec<Arc<EventData>>,
    pub animations: Vec<Arc<Animation>>,
    pub width: f32,
    pub height: f32,
    /// Reference scale used by constraints and physics.
    pub reference_scale: f32,
    pub version: SmolStr,
    pub hash: SmolStr,
    /// Path to the images used by the skeleton, if nonessential data was exported.
    pub images_path: SmolStr,
    /// Path to the audio used by the skeleton, if nonessential data was exported.
    pub audio_path: SmolStr,
    pub fps: f32,
}

impl SkeletonData {
    pub fn new() -> Self {
        SkeletonData {
            reference_scale: 100.0,
            fps: 30.0,
            ..Default::default()
        }
    }

    pub fn find_bone(&self, name: &str) -> Option<&BoneData> {
        if name.is_empty() {
            return None;
        }
        self.bones.iter().find(|b| b.name == name)
    }

    pub fn find_skin(&self, name: &str) -> Option<&Arc<Skin>> {
        if name.is_empty() {
            return None;
        }
        self.skins.iter().find(|s| s.name() == name)
    }

    /// Index of the constraint with the given name.
    pub fn find_constraint(&self, name: &str) -> Option<usize> {
        self.constraints.iter().position(|c| c.name() == name)
    }

    pub fn find_event(&self, name: &str) -> Option<&Arc<EventData>> {
        self.events.iter().find(|e| e.name == name)
    }

    pub fn find_animation(&self, name: &str) -> Option<&Arc<Animation>> {
        self.animations.iter().find(|a| a.name() == name)
    }

    pub fn find_slot(&self, name: &str) -> Option<&SlotData> {
        if name.is_empty() {
            return None;
        }
        self.slots.iter().find(|s| s.name == name)
    }
}
