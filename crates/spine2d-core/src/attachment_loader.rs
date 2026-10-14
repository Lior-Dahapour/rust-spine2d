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

use crate::attachment::{
    BoundingBoxAttachment, ClippingAttachment, MeshAttachment, PathAttachment, PointAttachment,
    RegionAttachment,
};
use crate::sequence::Sequence;
use crate::skin::Skin;

/// Creates attachments while a skeleton is being loaded, so applications can resolve texture
/// regions (or substitute their own attachments). Returning `None` skips the attachment.
pub trait AttachmentLoader {
    fn new_region_attachment(
        &self,
        skin: &Skin,
        placeholder: &str,
        name: &str,
        path: &str,
        sequence: Sequence,
    ) -> Option<RegionAttachment>;

    fn new_mesh_attachment(
        &self,
        skin: &Skin,
        placeholder: &str,
        name: &str,
        path: &str,
        sequence: Sequence,
    ) -> Option<MeshAttachment>;

    fn new_bounding_box_attachment(
        &self,
        skin: &Skin,
        placeholder: &str,
        name: &str,
    ) -> Option<BoundingBoxAttachment>;

    fn new_path_attachment(
        &self,
        skin: &Skin,
        placeholder: &str,
        name: &str,
    ) -> Option<PathAttachment>;

    fn new_point_attachment(
        &self,
        skin: &Skin,
        placeholder: &str,
        name: &str,
    ) -> Option<PointAttachment>;

    fn new_clipping_attachment(
        &self,
        skin: &Skin,
        placeholder: &str,
        name: &str,
    ) -> Option<ClippingAttachment>;
}
