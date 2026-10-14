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

/// How a bone inherits its parent's world transform.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Inherit {
    #[default]
    Normal,
    OnlyTranslation,
    NoRotationOrReflection,
    NoScale,
    NoScaleOrReflection,
}

impl Inherit {
    /// From the binary format / frame value index; out-of-range values are `Normal`.
    pub fn from_index(index: usize) -> Inherit {
        match index {
            1 => Inherit::OnlyTranslation,
            2 => Inherit::NoRotationOrReflection,
            3 => Inherit::NoScale,
            4 => Inherit::NoScaleOrReflection,
            _ => Inherit::Normal,
        }
    }

    /// Unknown names fall back to `Normal`, like the reference.
    pub fn from_name(name: &str) -> Inherit {
        match name {
            "onlyTranslation" => Inherit::OnlyTranslation,
            "noRotationOrReflection" => Inherit::NoRotationOrReflection,
            "noScale" => Inherit::NoScale,
            "noScaleOrReflection" => Inherit::NoScaleOrReflection,
            _ => Inherit::Normal,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum BlendMode {
    #[default]
    Normal,
    Additive,
    Multiply,
    Screen,
}

impl BlendMode {
    /// From the binary format index; out-of-range values are `Normal`.
    pub fn from_index(index: usize) -> BlendMode {
        match index {
            1 => BlendMode::Additive,
            2 => BlendMode::Multiply,
            3 => BlendMode::Screen,
            _ => BlendMode::Normal,
        }
    }

    pub fn from_name(name: &str) -> BlendMode {
        match name {
            "additive" => BlendMode::Additive,
            "multiply" => BlendMode::Multiply,
            "screen" => BlendMode::Screen,
            _ => BlendMode::Normal,
        }
    }
}

/// Determines how physics and other non-deterministic updates are applied.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Physics {
    /// Physics are not updated or applied.
    None,
    /// Physics are reset to the current pose.
    Reset,
    /// Physics are updated and the pose from physics is applied.
    Update,
    /// Physics are not updated but the pose from physics is applied.
    Pose,
}
