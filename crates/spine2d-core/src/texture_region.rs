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

use smol_str::SmolStr;

/// Extra information for a region that was packed into an atlas page.
///
/// Mirrors the fields of the reference `AtlasRegion` that attachment UV math depends on.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct AtlasRegionInfo {
    pub name: SmolStr,
    pub index: i32,
    pub page_width: i32,
    pub page_height: i32,
    pub x: i32,
    pub y: i32,
    pub offset_x: f32,
    pub offset_y: f32,
    pub packed_width: i32,
    pub packed_height: i32,
    pub original_width: i32,
    pub original_height: i32,
    pub rotate: bool,
    /// Rotation of the packed region in degrees: 0, 90, 180 or 270.
    pub degrees: i32,
}

/// A rectangle of a texture, in UV coordinates.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct TextureRegion {
    /// Opaque handle to the renderer's texture (0 = none).
    pub renderer_object: usize,
    pub u: f32,
    pub v: f32,
    pub u2: f32,
    pub v2: f32,
    pub region_width: i32,
    pub region_height: i32,
    /// Set when the region comes from an atlas.
    pub atlas: Option<AtlasRegionInfo>,
}
