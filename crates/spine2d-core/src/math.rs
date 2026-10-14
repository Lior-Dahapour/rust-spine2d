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

//! Math helpers ported from `MathUtil`.
//!
//! Trigonometry uses the native `f32` functions. The reference's `MathUtil` calls `::sin(float)`
//! and friends, which resolve to the single-precision overloads with libc++ (macOS, Android,
//! iOS); results then match the C++ runtime bit-for-bit on those platforms. Other C++ standard
//! libraries may use double precision there and differ in the last bit.

pub const EPSILON: f32 = 0.00001;
pub const EPSILON_SQ: f32 = EPSILON * EPSILON;
pub const PI: f32 = std::f32::consts::PI;
pub const PI_2: f32 = PI * 2.0;
pub const INV_PI_2: f32 = 1.0 / PI_2;
pub const DEG_RAD: f32 = PI / 180.0;
pub const RAD_DEG: f32 = 180.0 / PI;

#[inline]
pub fn sin(radians: f32) -> f32 {
    radians.sin()
}

#[inline]
pub fn cos(radians: f32) -> f32 {
    radians.cos()
}

#[inline]
pub fn sin_deg(degrees: f32) -> f32 {
    sin(degrees * DEG_RAD)
}

#[inline]
pub fn cos_deg(degrees: f32) -> f32 {
    cos(degrees * DEG_RAD)
}

#[inline]
pub fn atan2(y: f32, x: f32) -> f32 {
    y.atan2(x)
}

/// `atan2` returning degrees.
#[inline]
pub fn atan2_deg(y: f32, x: f32) -> f32 {
    atan2(y, x) * RAD_DEG
}

#[inline]
pub fn sqrt(v: f32) -> f32 {
    v.sqrt()
}

#[inline]
pub fn acos(v: f32) -> f32 {
    v.acos()
}

#[inline]
pub fn pow(a: f32, b: f32) -> f32 {
    a.powf(b)
}

/// C `fmod`: result has the sign of `a`.
#[inline]
pub fn fmod(a: f32, b: f32) -> f32 {
    ((a as f64) % (b as f64)) as f32
}

#[inline]
pub fn ceil(v: f32) -> f32 {
    v.ceil()
}

#[inline]
pub fn abs(v: f32) -> f32 {
    v.abs()
}

/// Returns -1, 0 or 1.
#[inline]
pub fn sign(v: f32) -> f32 {
    if v < 0.0 {
        -1.0
    } else if v > 0.0 {
        1.0
    } else {
        0.0
    }
}

#[inline]
pub fn clamp(x: f32, lower: f32, upper: f32) -> f32 {
    if x < lower {
        lower
    } else if x > upper {
        upper
    } else {
        x
    }
}
