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

use crate::math::sqrt;

/// Takes a linear value in the range 0-1 and outputs a usually non-linear, interpolated value.
#[derive(Clone, Copy, Debug, Default)]
pub enum Interpolation {
    #[default]
    Linear,
    /// Aka "smoothstep".
    Smooth,
    /// Slow, then fast.
    SlowFast,
    /// Fast, then slow.
    FastSlow,
    Circle,
    /// An application-provided curve.
    Custom(fn(f32) -> f32),
}

impl Interpolation {
    /// `a` is an alpha value between 0 and 1.
    pub fn apply(self, a: f32) -> f32 {
        match self {
            Interpolation::Linear => a,
            Interpolation::Smooth => a * a * (3.0 - 2.0 * a),
            Interpolation::SlowFast => a * a,
            Interpolation::FastSlow => 1.0 - (a - 1.0) * (a - 1.0),
            Interpolation::Circle => {
                if a <= 0.5 {
                    let a = a * 2.0;
                    (1.0 - sqrt(1.0 - a * a)) / 2.0
                } else {
                    let a = (a - 1.0) * 2.0;
                    (sqrt(1.0 - a * a) + 1.0) / 2.0
                }
            }
            Interpolation::Custom(f) => f(a),
        }
    }

    /// Interpolates between `start` and `end`.
    pub fn apply_between(self, start: f32, end: f32, a: f32) -> f32 {
        start + (end - start) * self.apply(a)
    }

    pub(crate) fn is_linear(self) -> bool {
        matches!(self, Interpolation::Linear)
    }
}
