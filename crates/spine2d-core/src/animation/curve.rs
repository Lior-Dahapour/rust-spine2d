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

use super::{search_step, MixFrom};
use crate::math::{abs, sign};

pub const LINEAR: f32 = 0.0;
pub const STEPPED: f32 = 1.0;
pub const BEZIER: f32 = 2.0;
/// Floats stored per bezier curve: 9 sampled (x, y) points.
pub const BEZIER_SIZE: usize = 18;

/// Frames plus per-frame interpolation curves, shared by every curve-based timeline.
///
/// `frames` holds `entries` floats per frame, the first being the time. `curves` holds one entry
/// per frame (`LINEAR`, `STEPPED`, or `BEZIER + offset` into the bezier samples that follow).
#[derive(Clone, Debug)]
pub struct CurveTimeline {
    pub frames: Vec<f32>,
    pub curves: Vec<f32>,
    entries: usize,
}

impl CurveTimeline {
    pub fn new(frame_count: usize, frame_entries: usize, bezier_count: usize) -> Self {
        let mut curves = vec![0.0; frame_count + bezier_count * BEZIER_SIZE];
        if frame_count > 0 {
            curves[frame_count - 1] = STEPPED;
        }
        CurveTimeline {
            frames: vec![0.0; frame_count * frame_entries],
            curves,
            entries: frame_entries,
        }
    }

    #[inline]
    pub fn frame_entries(&self) -> usize {
        self.entries
    }

    #[inline]
    pub fn frame_count(&self) -> usize {
        self.frames.len() / self.entries
    }

    /// Makes the curve from `frame` to the next frame linear. Don't call this for the last frame:
    /// it has no next frame and stays stepped.
    pub fn set_linear(&mut self, frame: usize) {
        self.curves[frame] = LINEAR;
    }

    pub fn set_stepped(&mut self, frame: usize) {
        self.curves[frame] = STEPPED;
    }

    /// Samples a cubic bezier into this timeline's curve storage.
    ///
    /// `value` is the index of the value within the frame (0 records the curve offset for the
    /// frame; later values of multi-value timelines share it).
    #[allow(clippy::too_many_arguments)]
    pub fn set_bezier(
        &mut self,
        bezier: usize,
        frame: usize,
        value: usize,
        time1: f32,
        value1: f32,
        cx1: f32,
        cy1: f32,
        cx2: f32,
        cy2: f32,
        time2: f32,
        value2: f32,
    ) {
        let mut i = self.frame_count() + bezier * BEZIER_SIZE;
        if value == 0 {
            self.curves[frame] = BEZIER + i as f32;
        }
        // The reference mixes float and double literals here; do the same to stay bit-identical.
        let tmpx = ((time1 - cx1 * 2.0 + cx2) as f64 * 0.03) as f32;
        let tmpy = ((value1 - cy1 * 2.0 + cy2) as f64 * 0.03) as f32;
        let dddx = (((cx1 - cx2) * 3.0 - time1 + time2) as f64 * 0.006) as f32;
        let dddy = (((cy1 - cy2) * 3.0 - value1 + value2) as f64 * 0.006) as f32;
        let mut ddx = tmpx * 2.0 + dddx;
        let mut ddy = tmpy * 2.0 + dddy;
        let mut dx = ((cx1 - time1) as f64 * 0.3 + tmpx as f64 + dddx as f64 * 0.16666667) as f32;
        let mut dy = ((cy1 - value1) as f64 * 0.3 + tmpy as f64 + dddy as f64 * 0.16666667) as f32;
        let mut x = time1 + dx;
        let mut y = value1 + dy;
        let n = i + BEZIER_SIZE;
        while i < n {
            self.curves[i] = x;
            self.curves[i + 1] = y;
            dx += ddx;
            dy += ddy;
            ddx += dddx;
            ddy += dddy;
            x += dx;
            y += dy;
            i += 2;
        }
    }

    /// Interpolated value on the bezier curve starting at `curves[i]` for the frame at
    /// `frame_index`; `value_offset` selects which value of the frame.
    pub fn bezier_value(
        &self,
        time: f32,
        frame_index: usize,
        value_offset: usize,
        i: usize,
    ) -> f32 {
        // A fixed-size view lets the compiler check the length once instead of on every access.
        let segment: &[f32; BEZIER_SIZE] = self.curves[i..i + BEZIER_SIZE]
            .try_into()
            .expect("slice has BEZIER_SIZE elements");
        if segment[0] > time {
            let x = self.frames[frame_index];
            let y = self.frames[frame_index + value_offset];
            return y + (time - x) / (segment[0] - x) * (segment[1] - y);
        }
        for j in (2..BEZIER_SIZE).step_by(2) {
            if segment[j] >= time {
                let x = segment[j - 2];
                let y = segment[j - 1];
                return y + (time - x) / (segment[j] - x) * (segment[j + 1] - y);
            }
        }
        let frame_index = frame_index + self.entries;
        let x = segment[BEZIER_SIZE - 2];
        let y = segment[BEZIER_SIZE - 1];
        y + (time - x) / (self.frames[frame_index] - x)
            * (self.frames[frame_index + value_offset] - y)
    }

    /// Samples `N` values of a multi-value timeline (`entries == N + 1`) at `time`, which must
    /// not be before the first frame.
    #[inline]
    pub fn sample<const N: usize>(&self, time: f32) -> [f32; N] {
        debug_assert_eq!(self.entries, N + 1);
        self.sample_prefix(time)
    }

    /// Samples the first `N` values of each frame of a timeline with more entries than `N + 1`
    /// (eg the IK timeline, whose later entries are not interpolated).
    #[inline]
    pub fn sample_prefix<const N: usize>(&self, time: f32) -> [f32; N] {
        let entries = self.entries;
        let i = search_step(&self.frames, time, entries);
        let curve_type = self.curves[i / entries] as i32;
        let mut out = [0.0f32; N];
        if curve_type == LINEAR as i32 {
            let before = self.frames[i];
            let t = (time - before) / (self.frames[i + entries] - before);
            let from = &self.frames[i + 1..i + 1 + N];
            let to = &self.frames[i + entries + 1..i + entries + 1 + N];
            for ((v, &a), &b) in out.iter_mut().zip(from).zip(to) {
                *v = a + (b - a) * t;
            }
        } else if curve_type == STEPPED as i32 {
            out.copy_from_slice(&self.frames[i + 1..i + 1 + N]);
        } else {
            let base = curve_type as usize - BEZIER as usize;
            for (k, v) in out.iter_mut().enumerate() {
                *v = self.bezier_value(time, i, 1 + k, base + k * BEZIER_SIZE);
            }
        }
        out
    }

    // --- Single-value timelines (entries == 2) -------------------------------------------------

    /// Sets the time and value of a frame of a single-value timeline.
    pub fn set_frame1(&mut self, frame: usize, time: f32, value: f32) {
        self.frames[frame << 1] = time;
        self.frames[(frame << 1) + 1] = value;
    }

    /// The interpolated value of a single-value timeline.
    pub fn curve_value(&self, time: f32) -> f32 {
        let i = search_step(&self.frames, time, 2);
        let curve_type = self.curves[i >> 1] as i32;
        if curve_type == LINEAR as i32 {
            let frame = &self.frames[i..i + 4];
            let (before, value) = (frame[0], frame[1]);
            value + (time - before) / (frame[2] - before) * (frame[3] - value)
        } else if curve_type == STEPPED as i32 {
            self.frames[i + 1]
        } else {
            self.bezier_value(time, i, 1, curve_type as usize - BEZIER as usize)
        }
    }

    pub fn relative_value(
        &self,
        time: f32,
        alpha: f32,
        from: MixFrom,
        add: bool,
        current: f32,
        setup: f32,
    ) -> f32 {
        if time < self.frames[0] {
            return before_first_key(from, alpha, current, setup);
        }
        let value = self.curve_value(time);
        if from == MixFrom::Setup {
            setup + value * alpha
        } else {
            current + (if add { value } else { value + setup - current }) * alpha
        }
    }

    pub fn absolute_value(
        &self,
        time: f32,
        alpha: f32,
        from: MixFrom,
        add: bool,
        current: f32,
        setup: f32,
    ) -> f32 {
        if time < self.frames[0] {
            return before_first_key(from, alpha, current, setup);
        }
        let value = self.curve_value(time);
        self.absolute_blend(alpha, from, add, current, setup, value)
    }

    /// Like `absolute_value` but with an explicit key `value`.
    #[allow(clippy::too_many_arguments)]
    pub fn absolute_value_of(
        &self,
        time: f32,
        alpha: f32,
        from: MixFrom,
        add: bool,
        current: f32,
        setup: f32,
        value: f32,
    ) -> f32 {
        if time < self.frames[0] {
            return before_first_key(from, alpha, current, setup);
        }
        self.absolute_blend(alpha, from, add, current, setup, value)
    }

    #[inline]
    fn absolute_blend(
        &self,
        alpha: f32,
        from: MixFrom,
        add: bool,
        current: f32,
        setup: f32,
        value: f32,
    ) -> f32 {
        if from == MixFrom::Setup {
            setup + (if add { value } else { value - setup }) * alpha
        } else {
            current + (if add { value } else { value - current }) * alpha
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub fn scale_value(
        &self,
        time: f32,
        alpha: f32,
        from: MixFrom,
        add: bool,
        out: bool,
        current: f32,
        setup: f32,
    ) -> f32 {
        if time < self.frames[0] {
            return before_first_key(from, alpha, current, setup);
        }
        let value = self.curve_value(time) * setup;
        scale_blend(value, alpha, from, add, out, current, setup)
    }
}

/// Blends a scale `value` (already multiplied by setup) into `current`, preserving sign rules.
#[inline]
pub(crate) fn scale_blend(
    value: f32,
    alpha: f32,
    from: MixFrom,
    add: bool,
    out: bool,
    current: f32,
    setup: f32,
) -> f32 {
    if alpha == 1.0 && !add {
        return value;
    }
    let mut base = if from == MixFrom::Setup {
        setup
    } else {
        current
    };
    if add {
        return base + (value - setup) * alpha;
    }
    if out {
        return base + (abs(value) * sign(base) - base) * alpha;
    }
    base = abs(base) * sign(value);
    base + (value - base) * alpha
}

/// The value to use when `time` is before the first key.
#[inline]
pub fn before_first_key(from: MixFrom, alpha: f32, current: f32, setup: f32) -> f32 {
    match from {
        MixFrom::Setup => setup,
        MixFrom::First => current + (setup - current) * alpha,
        MixFrom::Current => current,
    }
}
