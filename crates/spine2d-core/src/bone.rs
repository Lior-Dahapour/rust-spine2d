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

use crate::color::Color;
use crate::enums::Inherit;
use crate::math::{self, DEG_RAD, EPSILON_SQ};
use glam::{Affine2, Mat2, Vec2};
use smallvec::SmallVec;
use smol_str::SmolStr;
use std::sync::atomic::{AtomicBool, Ordering};

static Y_DOWN: AtomicBool = AtomicBool::new(true);

/// Whether the world Y axis points down. Process-wide, like the reference runtime's
/// `Bone::setYDown`, and `true` by default as in the 4.3 reference. Call
/// `set_y_down(false)` once at startup for y-up engines.
pub fn is_y_down() -> bool {
    Y_DOWN.load(Ordering::Relaxed)
}

pub fn set_y_down(value: bool) {
    Y_DOWN.store(value, Ordering::Relaxed);
}

/// Index of a bone in `Skeleton::bones()` / `SkeletonData::bones()`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct BoneId(pub usize);

impl BoneId {
    #[inline]
    pub fn index(self) -> usize {
        self.0
    }
}

/// A bone's local pose.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BoneLocal {
    pub x: f32,
    pub y: f32,
    /// Degrees, counter clockwise.
    pub rotation: f32,
    pub scale_x: f32,
    pub scale_y: f32,
    pub shear_x: f32,
    pub shear_y: f32,
    pub inherit: Inherit,
}

impl Default for BoneLocal {
    fn default() -> Self {
        BoneLocal {
            x: 0.0,
            y: 0.0,
            rotation: 0.0,
            scale_x: 1.0,
            scale_y: 1.0,
            shear_x: 0.0,
            shear_y: 0.0,
            inherit: Inherit::Normal,
        }
    }
}

impl BoneLocal {
    pub fn set_position(&mut self, x: f32, y: f32) {
        self.x = x;
        self.y = y;
    }

    pub fn set_scale(&mut self, scale_x: f32, scale_y: f32) {
        self.scale_x = scale_x;
        self.scale_y = scale_y;
    }
}

/// The `[a b; c d]` world matrix plus translation of a bone.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct WorldMatrix {
    pub a: f32,
    pub b: f32,
    pub c: f32,
    pub d: f32,
    pub world_x: f32,
    pub world_y: f32,
}

impl WorldMatrix {
    pub fn to_affine2(self) -> Affine2 {
        Affine2::from_mat2_translation(
            Mat2::from_cols(Vec2::new(self.a, self.c), Vec2::new(self.b, self.d)),
            Vec2::new(self.world_x, self.world_y),
        )
    }
}

/// Skeleton state the bone math depends on. `scale_y` already includes the y-down flip.
#[derive(Clone, Copy, Debug)]
pub struct SkeletonTransform {
    pub x: f32,
    pub y: f32,
    pub scale_x: f32,
    pub scale_y: f32,
    /// The skeleton's current update counter.
    pub update: i32,
}

/// Setup data for a bone. Shared by all skeleton instances.
#[derive(Clone, Debug)]
pub struct BoneData {
    pub index: usize,
    pub name: SmolStr,
    pub parent: Option<usize>,
    pub length: f32,
    pub color: Color,
    /// Bone icon name as in the editor; empty if nonessential data was not exported.
    pub icon: SmolStr,
    pub icon_size: f32,
    pub icon_rotation: f32,
    pub visible: bool,
    pub skin_required: bool,
    pub setup_pose: BoneLocal,
}

impl BoneData {
    pub fn new(index: usize, name: impl Into<SmolStr>, parent: Option<usize>) -> Self {
        BoneData {
            index,
            name: name.into(),
            parent,
            length: 0.0,
            color: Color::new(0.61, 0.61, 0.61, 1.0),
            icon: SmolStr::default(),
            icon_size: 1.0,
            icon_rotation: 0.0,
            visible: true,
            skin_required: false,
            setup_pose: BoneLocal::default(),
        }
    }
}

/// The applied local pose and world transform of a bone.
///
/// If the world transform is changed, call [`BonePose::update_local_transform`] before using
/// the local transform.
#[derive(Clone, Copy, Debug, Default)]
pub struct BonePose {
    pub local: BoneLocal,
    pub a: f32,
    pub b: f32,
    pub world_x: f32,
    pub c: f32,
    pub d: f32,
    pub world_y: f32,
    /// Update counter at which the world transform was last valid (0 = never / reset).
    pub(crate) world: i32,
    /// Update counter at which the local transform became stale (0 = local is valid).
    pub(crate) local_stale: i32,
}

impl BonePose {
    /// Copies the local pose only; the world transform is recomputed.
    pub fn set(&mut self, other: &BonePose) {
        self.local = other.local;
    }

    pub fn set_local(&mut self, local: &BoneLocal) {
        self.local = *local;
    }

    pub fn world_matrix(&self) -> WorldMatrix {
        WorldMatrix {
            a: self.a,
            b: self.b,
            c: self.c,
            d: self.d,
            world_x: self.world_x,
            world_y: self.world_y,
        }
    }

    pub fn world_position(&self) -> Vec2 {
        Vec2::new(self.world_x, self.world_y)
    }

    /// World rotation of the X axis in degrees: the direction the bone points.
    pub fn world_rotation_x(&self) -> f32 {
        math::atan2(self.c, self.a) * math::RAD_DEG
    }

    pub fn world_rotation_y(&self) -> f32 {
        math::atan2(self.d, self.b) * math::RAD_DEG
    }

    pub fn world_scale_x(&self) -> f32 {
        math::sqrt(self.a * self.a + self.c * self.c)
    }

    pub fn world_scale_y(&self) -> f32 {
        math::sqrt(self.b * self.b + self.d * self.d)
    }

    /// Computes the world transform from the parent's applied world matrix (`None` for the root)
    /// and this local pose. Children are not updated.
    pub fn update_world_transform(&mut self, sk: &SkeletonTransform, parent: Option<WorldMatrix>) {
        if self.local_stale == sk.update {
            self.update_local_transform(sk, parent);
        } else {
            self.world = sk.update;
        }

        let l = self.local;
        let Some(p) = parent else {
            // Root bone.
            let (sx, sy) = (sk.scale_x, sk.scale_y);
            let rx = (l.rotation + l.shear_x) * DEG_RAD;
            let ry = (l.rotation + 90.0 + l.shear_y) * DEG_RAD;
            self.a = math::cos(rx) * l.scale_x * sx;
            self.b = math::cos(ry) * l.scale_y * sx;
            self.c = math::sin(rx) * l.scale_x * sy;
            self.d = math::sin(ry) * l.scale_y * sy;
            self.world_x = l.x * sx + sk.x;
            self.world_y = l.y * sy + sk.y;
            return;
        };

        let (mut pa, mut pb, mut pc, mut pd) = (p.a, p.b, p.c, p.d);
        self.world_x = pa * l.x + pb * l.y + p.world_x;
        self.world_y = pc * l.x + pd * l.y + p.world_y;

        match l.inherit {
            Inherit::Normal => {
                let rx = (l.rotation + l.shear_x) * DEG_RAD;
                let ry = (l.rotation + 90.0 + l.shear_y) * DEG_RAD;
                let la = math::cos(rx) * l.scale_x;
                let lb = math::cos(ry) * l.scale_y;
                let lc = math::sin(rx) * l.scale_x;
                let ld = math::sin(ry) * l.scale_y;
                self.a = pa * la + pb * lc;
                self.b = pa * lb + pb * ld;
                self.c = pc * la + pd * lc;
                self.d = pc * lb + pd * ld;
            }
            Inherit::OnlyTranslation => {
                let (sx, sy) = (sk.scale_x, sk.scale_y);
                let rx = (l.rotation + l.shear_x) * DEG_RAD;
                let ry = (l.rotation + 90.0 + l.shear_y) * DEG_RAD;
                self.a = math::cos(rx) * l.scale_x * sx;
                self.b = math::cos(ry) * l.scale_y * sx;
                self.c = math::sin(rx) * l.scale_x * sy;
                self.d = math::sin(ry) * l.scale_y * sy;
            }
            Inherit::NoRotationOrReflection => {
                let (sx, sy) = (sk.scale_x, sk.scale_y);
                let (sxi, syi) = (1.0 / sx, 1.0 / sy);
                pa *= sxi;
                pc *= syi;
                let mut s = pa * pa + pc * pc;
                let r;
                if s > EPSILON_SQ {
                    s = (pa * pd * syi - pb * sxi * pc).abs() / s;
                    pb = pc * s;
                    pd = pa * s;
                    r = l.rotation - math::atan2_deg(pc, pa);
                } else {
                    pa = 0.0;
                    pc = 0.0;
                    r = l.rotation - 90.0 + math::atan2_deg(pd, pb);
                }
                let rx = (r + l.shear_x) * DEG_RAD;
                let ry = (r + l.shear_y + 90.0) * DEG_RAD;
                let la = math::cos(rx) * l.scale_x;
                let lb = math::cos(ry) * l.scale_y;
                let lc = math::sin(rx) * l.scale_x;
                let ld = math::sin(ry) * l.scale_y;
                self.a = (pa * la - pb * lc) * sx;
                self.b = (pa * lb - pb * ld) * sx;
                self.c = (pc * la + pd * lc) * sy;
                self.d = (pc * lb + pd * ld) * sy;
            }
            Inherit::NoScale | Inherit::NoScaleOrReflection => {
                let (sx, sy) = (sk.scale_x, sk.scale_y);
                let (sxi, syi) = (1.0 / sx, 1.0 / sy);
                let r = l.rotation * DEG_RAD;
                let (cos_r, sin_r) = (math::cos(r), math::sin(r));
                let mut za = (pa * cos_r + pb * sin_r) * sxi;
                let mut zc = (pc * cos_r + pd * sin_r) * syi;
                let s = 1.0 / math::sqrt(za * za + zc * zc);
                za *= s;
                zc *= s;
                let mut zb = -zc;
                let mut zd = za;
                if l.inherit == Inherit::NoScale
                    && ((pa * pd - pb * pc < 0.0) != ((sx < 0.0) != (sy < 0.0)))
                {
                    zb = -zb;
                    zd = -zd;
                }
                let rx = l.shear_x * DEG_RAD;
                let ry = (90.0 + l.shear_y) * DEG_RAD;
                let la = math::cos(rx) * l.scale_x;
                let lb = math::cos(ry) * l.scale_y;
                let lc = math::sin(rx) * l.scale_x;
                let ld = math::sin(ry) * l.scale_y;
                self.a = (za * la + zb * lc) * sx;
                self.b = (za * lb + zb * ld) * sx;
                self.c = (zc * la + zd * lc) * sy;
                self.d = (zc * lb + zd * ld) * sy;
            }
        }
    }

    /// Computes the local transform from the world transform.
    ///
    /// Some information is ambiguous in the world transform (eg -1,-1 scale versus 180 rotation),
    /// so the result is equivalent to, but may not be identical to, the original local transform.
    pub fn update_local_transform(&mut self, sk: &SkeletonTransform, parent: Option<WorldMatrix>) {
        self.local_stale = 0;
        self.world = sk.update;

        let (sx, sy) = (sk.scale_x, sk.scale_y);
        let Some(p) = parent else {
            let (sxi, syi) = (1.0 / sx, 1.0 / sy);
            self.local.x = (self.world_x - sk.x) * sxi;
            self.local.y = (self.world_y - sk.y) * syi;
            self.set_local_rot(self.a * sxi, self.b * sxi, self.c * syi, self.d * syi, 0.0);
            return;
        };

        let (mut pa, pb, mut pc, pd) = (p.a, p.b, p.c, p.d);
        let pad = pa * pd - pb * pc;
        let pid = 1.0 / pad;
        let (ia, ib, ic, id) = (pd * pid, pb * pid, pc * pid, pa * pid);
        let (dx, dy) = (self.world_x - p.world_x, self.world_y - p.world_y);
        self.local.x = dx * ia - dy * ib;
        self.local.y = dy * id - dx * ic;

        let (a, b, c, d) = (self.a, self.b, self.c, self.d);
        match self.local.inherit {
            Inherit::Normal => {
                self.set_local_rot(
                    ia * a - ib * c,
                    ia * b - ib * d,
                    id * c - ic * a,
                    id * d - ic * b,
                    0.0,
                );
            }
            Inherit::OnlyTranslation => {
                let (sxi, syi) = (1.0 / sx, 1.0 / sy);
                self.set_local_rot(a * sxi, b * sxi, c * syi, d * syi, 0.0);
            }
            Inherit::NoRotationOrReflection => {
                let (sxi, syi) = (1.0 / sx, 1.0 / sy);
                pa *= sxi;
                pc *= syi;
                let (wa, wb, wc, wd) = (a * sxi, b * sxi, c * syi, d * syi);
                let s = 1.0 / (pa * pa + pc * pc);
                let det = 1.0 / (pad * sxi * syi).abs();
                self.set_local_rot(
                    (pa * wa + pc * wc) * s,
                    (pa * wb + pc * wd) * s,
                    (pa * wc - pc * wa) * det,
                    (pa * wd - pc * wb) * det,
                    math::atan2_deg(pc, pa),
                );
            }
            Inherit::NoScale | Inherit::NoScaleOrReflection => {
                let (sxi, syi) = (1.0 / sx, 1.0 / sy);
                let (wa, wb, wc, wd) = (a * sxi, b * sxi, c * syi, d * syi);
                let mut tx = pd * a - pb * c;
                let mut ty = pa * c - pc * a;
                if pad < 0.0 {
                    tx = -tx;
                    ty = -ty;
                }
                let r = math::atan2_deg(ty, tx);
                self.local.rotation = r;
                let r = r * DEG_RAD;
                let (cos_r, sin_r) = (math::cos(r), math::sin(r));
                let mut za = (pa * cos_r + pb * sin_r) * sxi;
                let mut zc = (pc * cos_r + pd * sin_r) * syi;
                let s = 1.0 / math::sqrt(za * za + zc * zc);
                za *= s;
                zc *= s;
                let si = if self.local.inherit == Inherit::NoScale
                    && ((pad < 0.0) != ((sx < 0.0) != (sy < 0.0)))
                {
                    -1.0
                } else {
                    1.0
                };
                self.set_local_shear(
                    za * wa + zc * wc,
                    za * wb + zc * wd,
                    (za * wc - zc * wa) * si,
                    (za * wd - zc * wb) * si,
                );
            }
        }
    }

    /// Sets scale and shear (not rotation) from a local matrix.
    fn set_local_shear(&mut self, ra: f32, rb: f32, rc: f32, rd: f32) {
        let l = &mut self.local;
        let x = ra * ra + rc * rc;
        let y = rb * rb + rd * rd;
        if x > EPSILON_SQ {
            l.shear_x = math::atan2_deg(rc, ra);
            l.scale_x = math::sqrt(x);
        } else {
            l.shear_x = 0.0;
            l.scale_x = 0.0;
        }
        l.scale_y = math::sqrt(y);
        if y > EPSILON_SQ {
            l.shear_y = math::atan2_deg(rd, rb);
            if ra * rd - rb * rc < 0.0 {
                l.scale_y = -l.scale_y;
                l.shear_y += 90.0;
            } else {
                l.shear_y -= 90.0;
            }
            if l.shear_y > 180.0 {
                l.shear_y -= 360.0;
            } else if l.shear_y <= -180.0 {
                l.shear_y += 360.0;
            }
        } else {
            l.shear_y = 0.0;
        }
    }

    /// Sets rotation, scale and shear from a local matrix, adding `ro` degrees to the rotation.
    fn set_local_rot(&mut self, ra: f32, rb: f32, rc: f32, rd: f32, ro: f32) {
        let l = &mut self.local;
        l.shear_x = 0.0;
        let x = ra * ra + rc * rc;
        let y = rb * rb + rd * rd;
        if x > EPSILON_SQ {
            let r = math::atan2_deg(rc, ra);
            l.rotation = r + ro;
            l.scale_x = math::sqrt(x);
            l.scale_y = math::sqrt(y);
            if y > EPSILON_SQ {
                l.shear_y = math::atan2_deg(rd, rb);
                if ra * rd - rb * rc < 0.0 {
                    l.scale_y = -l.scale_y;
                    l.shear_y += 90.0 - r;
                } else {
                    l.shear_y -= 90.0 + r;
                }
                if l.shear_y > 180.0 {
                    l.shear_y -= 360.0;
                } else if l.shear_y <= -180.0 {
                    l.shear_y += 360.0;
                }
            } else {
                l.shear_y = 0.0;
            }
        } else {
            l.scale_x = 0.0;
            l.scale_y = math::sqrt(y);
            l.shear_y = 0.0;
            l.rotation = if y > EPSILON_SQ {
                math::atan2_deg(rd, rb) - 90.0 + ro
            } else {
                ro
            };
        }
    }

    /// Transforms a point from world coordinates to this bone's local coordinates.
    pub fn world_to_local(&self, world_x: f32, world_y: f32) -> (f32, f32) {
        let det = self.a * self.d - self.b * self.c;
        let x = world_x - self.world_x;
        let y = world_y - self.world_y;
        (
            (x * self.d - y * self.b) / det,
            (y * self.a - x * self.c) / det,
        )
    }

    /// Transforms a point from this bone's local coordinates to world coordinates.
    pub fn local_to_world(&self, local_x: f32, local_y: f32) -> (f32, f32) {
        (
            local_x * self.a + local_y * self.b + self.world_x,
            local_x * self.c + local_y * self.d + self.world_y,
        )
    }

    /// Transforms a world rotation (degrees) to a local rotation.
    pub fn world_to_local_rotation(&self, world_rotation: f32) -> f32 {
        let r = world_rotation * DEG_RAD;
        let (sin_r, cos_r) = (math::sin(r), math::cos(r));
        math::atan2(
            self.a * sin_r - self.c * cos_r,
            self.d * cos_r - self.b * sin_r,
        ) * math::RAD_DEG
            + self.local.rotation
            - self.local.shear_x
    }

    /// Transforms a local rotation (degrees) to a world rotation.
    pub fn local_to_world_rotation(&self, local_rotation: f32) -> f32 {
        let r = (local_rotation - self.local.rotation - self.local.shear_x) * DEG_RAD;
        let (sin_r, cos_r) = (math::sin(r), math::cos(r));
        math::atan2(
            cos_r * self.c + sin_r * self.d,
            cos_r * self.a + sin_r * self.b,
        ) * math::RAD_DEG
    }

    /// Rotates the world transform by `degrees`.
    pub fn rotate_world(&mut self, degrees: f32) {
        let r = degrees * DEG_RAD;
        let (sin_r, cos_r) = (math::sin(r), math::cos(r));
        let (ra, rb) = (self.a, self.b);
        self.a = cos_r * ra - sin_r * self.c;
        self.b = cos_r * rb - sin_r * self.d;
        self.c = sin_r * ra + cos_r * self.c;
        self.d = sin_r * rb + cos_r * self.d;
    }
}

/// Which of a bone's two poses is used for rendering.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Applied {
    /// The unconstrained pose, set by animations and application code.
    Pose,
    /// The pose copy modified by constraints.
    Constrained,
}

/// A bone instance within a skeleton: the three poses plus hierarchy and update-cache state.
#[derive(Clone, Debug)]
pub struct Bone {
    pub(crate) index: usize,
    pub(crate) parent: Option<BoneId>,
    pub(crate) children: SmallVec<[BoneId; 4]>,
    /// The unconstrained pose, set by animations and application code.
    pub pose: BonePose,
    pub(crate) constrained: BonePose,
    pub(crate) applied: Applied,
    pub(crate) active: bool,
    pub(crate) sorted: bool,
}

impl Bone {
    pub(crate) fn new(data: &BoneData, parent: Option<BoneId>) -> Bone {
        let mut bone = Bone {
            index: data.index,
            parent,
            children: SmallVec::new(),
            pose: BonePose::default(),
            constrained: BonePose::default(),
            applied: Applied::Pose,
            active: true,
            sorted: false,
        };
        bone.setup_pose(data);
        bone
    }

    pub fn id(&self) -> BoneId {
        BoneId(self.index)
    }

    pub fn parent(&self) -> Option<BoneId> {
        self.parent
    }

    pub fn children(&self) -> &[BoneId] {
        &self.children
    }

    /// False when skin-required and the active skin does not include this bone.
    pub fn is_active(&self) -> bool {
        self.active
    }

    /// The pose to use for rendering: `pose`, or the constrained copy if constraints modify it.
    pub fn applied_pose(&self) -> &BonePose {
        match self.applied {
            Applied::Pose => &self.pose,
            Applied::Constrained => &self.constrained,
        }
    }

    pub fn applied_pose_mut(&mut self) -> &mut BonePose {
        match self.applied {
            Applied::Pose => &mut self.pose,
            Applied::Constrained => &mut self.constrained,
        }
    }

    pub fn is_pose_equal_to_applied(&self) -> bool {
        self.applied == Applied::Pose
    }

    pub(crate) fn setup_pose(&mut self, data: &BoneData) {
        self.pose.local = data.setup_pose;
    }

    pub(crate) fn reset_constrained(&mut self) {
        self.constrained.local = self.pose.local;
    }

    pub(crate) fn constrained(&mut self) {
        self.applied = Applied::Constrained;
    }

    pub(crate) fn unconstrained(&mut self) {
        self.applied = Applied::Pose;
    }
}
