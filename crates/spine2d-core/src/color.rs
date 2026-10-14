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

use crate::math::clamp;
use glam::Vec4;

/// RGBA color with components clamped to 0..=1, like the reference `Color`.
#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub struct Color {
    pub r: f32,
    pub g: f32,
    pub b: f32,
    pub a: f32,
}

impl Color {
    pub const WHITE: Color = Color {
        r: 1.0,
        g: 1.0,
        b: 1.0,
        a: 1.0,
    };

    pub fn new(r: f32, g: f32, b: f32, a: f32) -> Self {
        Color { r, g, b, a }.clamped()
    }

    pub fn set(&mut self, r: f32, g: f32, b: f32, a: f32) -> &mut Self {
        *self = Color { r, g, b, a }.clamped();
        self
    }

    pub fn set_rgb(&mut self, r: f32, g: f32, b: f32) -> &mut Self {
        self.set(r, g, b, self.a)
    }

    pub fn add(&mut self, r: f32, g: f32, b: f32, a: f32) -> &mut Self {
        self.set(self.r + r, self.g + g, self.b + b, self.a + a)
    }

    pub fn clamped(self) -> Self {
        Color {
            r: clamp(self.r, 0.0, 1.0),
            g: clamp(self.g, 0.0, 1.0),
            b: clamp(self.b, 0.0, 1.0),
            a: clamp(self.a, 0.0, 1.0),
        }
    }

    /// Sets from `RRGGBB` or `RRGGBBAA` hex without clamping, like the reference
    /// `Color::valueOf`. Does nothing if the string has fewer than 6 characters.
    pub fn set_hex(&mut self, hex: &str) {
        let b = hex.as_bytes();
        if b.len() < 6 {
            return;
        }
        let byte = |i: usize| -> f32 {
            let digits = hex.get(i * 2..i * 2 + 2).unwrap_or("");
            u32::from_str_radix(digits, 16).unwrap_or(0) as f32 / 255.0
        };
        self.r = byte(0);
        self.g = byte(1);
        self.b = byte(2);
        self.a = if b.len() >= 8 { byte(3) } else { 1.0 };
    }

    /// From a packed `0xRRGGBBAA` value.
    pub fn from_rgba8888(value: i32) -> Color {
        let v = value as u32;
        Color {
            r: ((v & 0xff00_0000) >> 24) as f32 / 255.0,
            g: ((v & 0x00ff_0000) >> 16) as f32 / 255.0,
            b: ((v & 0x0000_ff00) >> 8) as f32 / 255.0,
            a: (v & 0x0000_00ff) as f32 / 255.0,
        }
    }

    /// From a packed `0xRRGGBB` value, with alpha 1.
    pub fn from_rgb888(value: i32) -> Color {
        let v = value as u32;
        Color {
            r: ((v & 0xff_0000) >> 16) as f32 / 255.0,
            g: ((v & 0x00_ff00) >> 8) as f32 / 255.0,
            b: (v & 0x00_00ff) as f32 / 255.0,
            a: 1.0,
        }
    }

    /// Parses `RRGGBB` or `RRGGBBAA` hex. Returns `None` if shorter than 6 digits or not hex.
    pub fn from_hex(hex: &str) -> Option<Color> {
        let hex = hex.trim_start_matches('#');
        if hex.len() < 6 || !hex.is_ascii() {
            return None;
        }
        let byte = |i: usize| -> Option<f32> {
            u8::from_str_radix(hex.get(i * 2..i * 2 + 2)?, 16)
                .ok()
                .map(|v| v as f32 / 255.0)
        };
        let a = if hex.len() >= 8 { byte(3)? } else { 1.0 };
        Some(Color {
            r: byte(0)?,
            g: byte(1)?,
            b: byte(2)?,
            a,
        })
    }
}

impl From<Color> for Vec4 {
    fn from(c: Color) -> Vec4 {
        Vec4::new(c.r, c.g, c.b, c.a)
    }
}

impl From<Vec4> for Color {
    fn from(v: Vec4) -> Color {
        Color::new(v.x, v.y, v.z, v.w)
    }
}
