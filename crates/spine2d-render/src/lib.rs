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

//! Engine-agnostic rendering support for Spine skeletons.
//!
//! - [`SkeletonRenderer`] turns a posed [`Skeleton`](spine2d_core::Skeleton) into batched
//!   [`RenderCommand`]s (positions, UVs, packed colors, indices, blend mode and texture handle)
//!   that any engine integration can submit.
//! - [`SkeletonClipping`] and [`Triangulator`] implement clipping attachments.
//! - [`compute_bounds`] and [`SkeletonBounds`] provide the drawn AABB and bounding box hit tests.
//!
//! Everything is checked against the C++ reference runtime; see `tests/golden_render.rs`.

pub mod bounds;
pub mod clipping;
pub mod renderer;
pub mod triangulator;

pub use bounds::{compute_bounds, Bounds, Polygon, SkeletonBounds};
pub use clipping::SkeletonClipping;
pub use renderer::{BatchGroup, Batches, RenderCommand, SkeletonRenderer, SlotMarker};
pub use triangulator::Triangulator;
